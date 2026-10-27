//! Git worktree helpers for spawn flow (explicit opt-in; no orch bridge).

use std::path::{Path, PathBuf};
use std::process::Command;

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
            WorktreeError::Git(format!("create parent dir: {e}"))
        })?;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(new_branch)
        .arg(dest)
        .arg(base_branch)
        .output()
        .map_err(|e| WorktreeError::Git(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(WorktreeError::Git(stderr.trim().to_string()))
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

fn git_stdout(path: &Path, args: &[&str]) -> Option<String> {
    git_output(path, args).ok()
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
    fn slug_sanitizes_path_segment() {
        let dest = resolve_worktree_dest(Path::new("/tmp/wt"), "feat/foo bar");
        assert_eq!(dest, PathBuf::from("/tmp/wt/feat-foo-bar"));
    }
}
