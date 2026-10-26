//! Session metadata for Fleet preview header (git + tmux).

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionChrome {
    pub repo: String,
    pub worktree: String,
    pub branch: String,
    pub agent: String,
    pub uptime: String,
    pub context: String,
    pub tasks: String,
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
            git_context(Path::new(path))
        };
        Self {
            repo,
            worktree,
            branch,
            agent: tmux_agent_label(session_name),
            uptime: tmux_session_uptime(session_name),
            context: "n/a".into(),
            tasks: "n/a".into(),
        }
    }

    pub fn lines(&self) -> Vec<String> {
        vec![
            format!(
                "repo: {}  │  worktree: {}  │  branch: {}",
                self.repo,
                self.worktree,
                self.branch
            ),
            format!(
                "agent: {}  │  up: {}  │  context: {}  │  tasks: {}",
                self.agent,
                self.uptime,
                self.context,
                self.tasks
            ),
        ]
    }
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

fn tmux_agent_label(session_name: &str) -> String {
    for key in ["@cdamux_agent", "@ccmux_agent"] {
        if let Some(v) = tmux_display(session_name, key) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    "—".into()
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
}
