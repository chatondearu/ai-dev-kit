//! Session metadata for Fleet preview header (git + tmux).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GIT_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionChrome {
    pub repo: String,
    pub worktree: String,
    pub branch: String,
    pub agent: String,
    pub uptime: String,
    pub context: String,
    pub tasks: String,
    pub issue: Option<u32>,
    pub cwd: String,
}

impl SessionChrome {
    pub fn gather(session_name: &str, cwd: &str) -> Self {
        let path = cwd.trim();
        let (repo, worktree, branch) = if path.is_empty() {
            (
                "—".into(),
                "—".into(),
                "—".into(),
            )
        } else {
            git_context_cached(Path::new(path))
        };
        let issue = crate::issue_ref::issue_from_branch_or_path(&branch, path);
        Self {
            repo,
            worktree,
            branch,
            agent: session_agent_label(session_name),
            uptime: tmux_session_uptime(session_name),
            context: "n/a".into(),
            tasks: "n/a".into(),
            issue,
            cwd: path.to_string(),
        }
    }

    /// Refresh tmux-derived fields without re-running git (preview poll interval).
    pub fn refresh_volatile(&mut self, session_name: &str) {
        self.agent = session_agent_label(session_name);
        self.uptime = tmux_session_uptime(session_name);
    }

    pub fn lines(&self) -> Vec<String> {
        let mut branch_line = format!(
            "repo: {}  │  worktree: {}  │  branch: {}",
            self.repo, self.worktree, self.branch
        );
        if let Some(n) = self.issue {
            branch_line.push_str(&format!("  │  issue: #{n} (o = open)"));
        }
        vec![
            branch_line,
            format!(
                "agent: {}  │  up: {}  │  context: {}  │  tasks: {}",
                self.agent, self.uptime, self.context, self.tasks
            ),
        ]
    }
}

/// Current branch for a cwd (cached with other git chrome fields).
pub fn branch_for_cwd(cwd: &str) -> String {
    let path = cwd.trim();
    if path.is_empty() {
        return "—".into();
    }
    git_context_cached(Path::new(path)).2
}

fn git_context_cached(path: &Path) -> (String, String, String) {
    static CACHE: LazyLock<Mutex<HashMap<PathBuf, (Instant, String, String, String)>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Ok(mut cache) = CACHE.lock() {
        if let Some((at, repo, wt, br)) = cache.get(&key) {
            if at.elapsed() < GIT_CACHE_TTL {
                return (repo.clone(), wt.clone(), br.clone());
            }
        }
        let triple = git_context(path);
        cache.insert(key, (Instant::now(), triple.0.clone(), triple.1.clone(), triple.2.clone()));
        return triple;
    }
    git_context(path)
}

fn git_context(path: &Path) -> (String, String, String) {
    let root = git_output(path, &["rev-parse", "--show-toplevel"]);
    let branch = git_output(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "—".to_string());
    let repo = root
        .as_ref()
        .and_then(|r| Path::new(r).file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    let worktree = match git_output(path, &["rev-parse", "--git-dir"]) {
        Some(git_dir) if git_dir.contains("worktrees") => git_dir
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty() && *s != "worktrees")
            .unwrap_or("linked")
            .to_string(),
        Some(_) => "main".into(),
        None => "—".into(),
    };

    (repo, worktree, branch)
}

fn git_output(path: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(path);
    cmd.args(args);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Placeholder when neither `@cdamux_agent` nor legacy `@ccmux_agent` is set.
pub const AGENT_LABEL_MISSING: &str = "—";

/// Prefer first non-empty option value; else [`AGENT_LABEL_MISSING`].
pub fn resolve_agent_label(primary: Option<&str>, legacy: Option<&str>) -> String {
    for v in [primary, legacy].into_iter().flatten() {
        let t = v.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    AGENT_LABEL_MISSING.into()
}

/// Agent id from tmux user options (`@cdamux_agent`, legacy `@ccmux_agent`).
pub fn session_agent_label(session_name: &str) -> String {
    resolve_agent_label(
        tmux_display(session_name, "@cdamux_agent").as_deref(),
        tmux_display(session_name, "@ccmux_agent").as_deref(),
    )
}

fn tmux_session_uptime(session_name: &str) -> String {
    let created = tmux_display(session_name, "#{session_created}");
    let secs = created.and_then(|s| s.parse::<u64>().ok());
    match secs {
        Some(ts) => format_duration_since(ts),
        None => "—".into(),
    }
}

fn tmux_display(session_name: &str, format: &str) -> Option<String> {
    let out = Command::new("tmux")
        .args(["display-message", "-p", "-t", session_name, format])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn format_duration_since(unix_secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let elapsed = now.saturating_sub(unix_secs);
    if elapsed < 60 {
        format!("{}s", elapsed)
    } else if elapsed < 3600 {
        format!("{}m", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{}h", elapsed / 3600)
    } else {
        format!("{}d", elapsed / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formats() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(format_duration_since(now).ends_with('s'));
        assert_eq!(format_duration_since(now - 120), "2m");
    }

    #[test]
    fn resolve_agent_label_prefers_primary_then_legacy_then_placeholder() {
        assert_eq!(resolve_agent_label(Some("claude"), Some("cursor")), "claude");
        assert_eq!(resolve_agent_label(None, Some("cursor")), "cursor");
        assert_eq!(resolve_agent_label(Some("  "), Some("pi")), "pi");
        assert_eq!(resolve_agent_label(None, None), AGENT_LABEL_MISSING);
        assert_eq!(resolve_agent_label(Some(""), Some("")), AGENT_LABEL_MISSING);
    }
}
