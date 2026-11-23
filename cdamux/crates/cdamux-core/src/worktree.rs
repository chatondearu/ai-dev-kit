//! Git worktree helpers for spawn flow (explicit opt-in; no orch bridge).

use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Hang guard: a full disk blocks `git worktree add` instead of returning ENOSPC.
/// shortcut: 30s, raise if a real checkout of a large repo exceeds it.
const WORKTREE_ADD_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub branch: String,
    pub bare: bool,
}

#[derive(Debug, Clone)]
pub enum WorktreeError {
    Git(String),
    Config(String),
}

impl std::fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktreeError::Git(s) => write!(f, "git: {s}"),
            WorktreeError::Config(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for WorktreeError {}

pub fn git_toplevel(path: &Path) -> Option<PathBuf> {
    git_stdout(path, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

pub fn list_local_branches(repo: &Path) -> Vec<String> {
    let out = git_stdout(repo, &["branch", "--format=%(refname:short)"]);
    match out {
        Some(s) => s
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

pub fn current_branch(repo: &Path) -> Option<String> {
    git_stdout(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Default branch of `repo` (`origin/HEAD`, else `gh repo view`); `None` when unknown.
pub fn default_branch(repo: &Path) -> Option<String> {
    git_stdout(repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .map(|r| r.trim_start_matches("origin/").to_string())
        .or_else(|| {
            let out = Command::new("gh")
                .args(["repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"])
                .current_dir(repo)
                .output()
                .ok()?;
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (out.status.success() && !b.is_empty()).then_some(b)
        })
}

pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeEntry>, WorktreeError> {
    let out = git_output(repo, &["worktree", "list", "--porcelain"])?;
    let mut entries = Vec::new();
    let mut path = None;
    let mut branch = String::new();
    let mut bare = false;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(prev) = path.take() {
                entries.push(WorktreeEntry {
                    path: prev,
                    branch: branch.clone(),
                    bare,
                });
            }
            path = Some(PathBuf::from(p));
            branch.clear();
            bare = false;
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = b.trim_start_matches("refs/heads/").to_string();
        } else if line == "bare" {
            bare = true;
        } else if line.is_empty() {
            if let Some(prev) = path.take() {
                entries.push(WorktreeEntry {
                    path: prev,
                    branch: branch.clone(),
                    bare,
                });
                branch.clear();
                bare = false;
            }
        }
    }
    if let Some(prev) = path {
        entries.push(WorktreeEntry {
            path: prev,
            branch,
            bare,
        });
    }
    Ok(entries)
}

/// Create a linked worktree with a new branch based on `base_branch`.
pub fn worktree_add_new_branch(
    repo: &Path,
    dest: &Path,
    new_branch: &str,
    base_branch: &str,
) -> Result<(), WorktreeError> {
    if new_branch.trim().is_empty() {
        return Err(WorktreeError::Config("new branch name is empty".into()));
    }
    if dest.exists() {
        return Err(WorktreeError::Git(format!(
            "path already exists: {}",
            dest.display()
        )));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            if e.kind() == ErrorKind::StorageFull {
                WorktreeError::Git("ENOSPC: no space left on device".into())
            } else {
                WorktreeError::Git(format!("create parent dir: {e}"))
            }
        })?;
    }
    // stdout is discarded so a chatty checkout cannot fill the pipe and deadlock the wait.
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(new_branch)
        .arg(dest)
        .arg(base_branch)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == ErrorKind::StorageFull {
                WorktreeError::Git("ENOSPC: no space left on device".into())
            } else {
                WorktreeError::Git(e.to_string())
            }
        })?;
    let status = wait_with_timeout(&mut child, WORKTREE_ADD_TIMEOUT)?;
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    if status.success() {
        Ok(())
    } else {
        Err(WorktreeError::Git(git_err_line(&stderr)))
    }
}

/// Best-effort undo of [`worktree_add_new_branch`] after a later spawn step failed
/// (M10): remove the worktree (or what a timed-out add left), prune, delete the branch.
/// Only for a `dest` this process just created; errors are ignored.
pub fn remove_new_worktree(repo: &Path, dest: &Path, branch: &str) {
    let git = |args: &[&str]| {
        let _ = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    git(&["worktree", "remove", "--force", &dest.to_string_lossy()]);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(dest);
    }
    git(&["worktree", "prune"]);
    git(&["branch", "-D", branch]);
}

fn git_err_line(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines
        .iter()
        .copied()
        .find(|l| l.contains("No space left on device") || l.contains("ENOSPC"))
        .or_else(|| lines.first().copied())
        .unwrap_or("git worktree add failed")
        .to_string()
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, WorktreeError> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let kill_wait = Instant::now();
                while kill_wait.elapsed() < Duration::from_millis(200) {
                    if child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                return Err(WorktreeError::Git("git worktree add timed out".into()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(WorktreeError::Git(e.to_string())),
        }
    }
}

pub fn resolve_worktree_dest(root: &Path, slug: &str) -> PathBuf {
    let safe = slug
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    let name = if safe.is_empty() { "worktree" } else { safe.as_str() };
    root.join(name)
}

pub(crate) fn git_stdout(path: &Path, args: &[&str]) -> Option<String> {
    git_output(path, args).ok().filter(|s| !s.is_empty())
}

fn git_output(path: &Path, args: &[&str]) -> Result<String, WorktreeError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|e| WorktreeError::Git(e.to_string()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr.trim();
        return Err(WorktreeError::Git(
            if msg.is_empty() {
                "git command failed".into()
            } else {
                msg.to_string()
            },
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_add_enospc_is_one_line() {
        let err = git_err_line("hint: something\nfatal: No space left on device\n");
        assert_eq!(err, "fatal: No space left on device");
        assert!(!err.contains('\n'));
        assert_eq!(
            git_err_line("fatal: branch already exists\n"),
            "fatal: branch already exists"
        );
    }

    #[test]
    fn worktree_add_timeout_returns_one_line() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let err = wait_with_timeout(&mut child, Duration::from_millis(200)).unwrap_err();
        assert_eq!(err.to_string(), "git: git worktree add timed out");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn remove_new_worktree_drops_checkout_and_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git").arg("-C").arg(&repo).args(args).output().unwrap().status.success());
        };
        git(&["init", "-b", "main"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-m", "i"]);
        let dest = tmp.path().join("wt/w1");
        worktree_add_new_branch(&repo, &dest, "worker/r/w1", "main").unwrap();
        assert!(list_local_branches(&repo).contains(&"worker/r/w1".to_string()));
        remove_new_worktree(&repo, &dest, "worker/r/w1");
        assert!(!dest.exists());
        assert!(!list_local_branches(&repo).contains(&"worker/r/w1".to_string()));
        // Retry with the same name works again (M10).
        worktree_add_new_branch(&repo, &dest, "worker/r/w1", "main").unwrap();
    }

    #[test]
    fn slug_sanitizes_path_segment() {
        let dest = resolve_worktree_dest(Path::new("/tmp/wt"), "feat/foo bar");
        assert_eq!(dest, PathBuf::from("/tmp/wt/feat-foo-bar"));
    }
}
