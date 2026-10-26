//! Parse unified git diffs into hunks and format agent review prompts.

use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub path: String,
    /// `@@ … @@` header line.
    pub header: String,
    pub body: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DiffError {
    #[error("git not found on PATH")]
    GitNotFound,
    #[error("git diff failed in {0}")]
    GitFailed(String),
    #[error("no tmux session named '{0}'")]
    SessionNotFound(String),
    #[error("session '{0}' has no path metadata")]
    SessionPathMissing(String),
    #[error("repo path does not exist: {0}")]
    PathNotFound(String),
}

/// Run `git diff` and `git diff --cached` in `repo` and concatenate output.
pub fn git_diff_unified(repo: &Path) -> Result<String, DiffError> {
    if !repo.is_dir() {
        return Err(DiffError::PathNotFound(repo.display().to_string()));
    }
    let unstaged = run_git_diff(repo, &["diff"])?;
    let staged = run_git_diff(repo, &["diff", "--cached"])?;
    if unstaged.is_empty() {
        Ok(staged)
    } else if staged.is_empty() {
        Ok(unstaged)
    } else {
        Ok(format!("{}\n{}", unstaged.trim_end(), staged))
    }
}

fn run_git_diff(repo: &Path, args: &[&str]) -> Result<String, DiffError> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DiffError::GitNotFound
            } else {
                DiffError::GitFailed(e.to_string())
            }
        })?;
    if !output.status.success() {
        return Err(DiffError::GitFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse a unified diff into per-hunk records (one entry per `@@` block).
pub fn parse_unified_diff(text: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut file_path = String::new();
    let mut header = String::new();
    let mut body = String::new();

    let mut flush_hunk = |file_path: &mut String, header: &mut String, body: &mut String| {
        if header.is_empty() {
            return;
        }
        hunks.push(DiffHunk {
            path: file_path.clone(),
            header: header.clone(),
            body: body.trim_end().to_string(),
        });
        header.clear();
        body.clear();
    };

    for line in text.lines() {
        if line.starts_with("diff --git ") {
            flush_hunk(&mut file_path, &mut header, &mut body);
            file_path = path_from_diff_git(line);
            continue;
        }
        if line.starts_with("+++ ") {
            let p = line.strip_prefix("+++ ").unwrap_or(line);
            let p = p.strip_prefix("b/").unwrap_or(p);
            if p != "/dev/null" && !p.is_empty() {
                file_path = p.to_string();
            }
            continue;
        }
        if line.starts_with("@@ ") {
            flush_hunk(&mut file_path, &mut header, &mut body);
            header = line.to_string();
            continue;
        }
        if !header.is_empty() {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line);
        }
    }
    flush_hunk(&mut file_path, &mut header, &mut body);
    hunks
}

fn path_from_diff_git(line: &str) -> String {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let path = parts.last().map(|s| *s).unwrap_or("");
    path.strip_prefix("b/").unwrap_or(path).to_string()
}

/// Build the message sent to the agent session (plain text).
pub fn format_review_prompt(instruction: &str, hunks: &[DiffHunk]) -> String {
    let mut out = String::new();
    let lead = instruction.trim();
    if !lead.is_empty() {
        out.push_str(lead);
        out.push_str("\n\n");
    }
    for h in hunks {
        out.push_str("### ");
        out.push_str(&h.path);
        out.push('\n');
        out.push_str(&h.header);
        out.push('\n');
        if !h.body.is_empty() {
            out.push_str(&h.body);
            if !h.body.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 111..222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!(\"hi\");
 }
@@ -10,2 +11,3 @@ fn other() {
     x
+    y
 }
";

    #[test]
    fn parse_two_hunks_same_file() {
        let hunks = parse_unified_diff(SAMPLE);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].path, "src/lib.rs");
        assert!(hunks[0].header.starts_with("@@"));
        assert!(hunks[0].body.contains("println"));
        assert!(hunks[1].body.contains("y"));
    }

    #[test]
    fn format_prompt_includes_instruction_and_hunks() {
        let hunks = parse_unified_diff(SAMPLE);
        let msg = format_review_prompt("Review please.", &hunks[..1]);
        assert!(msg.starts_with("Review please."));
        assert!(msg.contains("### src/lib.rs"));
        assert!(msg.contains("@@"));
    }
}
