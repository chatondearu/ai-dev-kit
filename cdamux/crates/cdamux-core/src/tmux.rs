//! Parse and list tmux sessions (CLI subprocess v0).

use crate::session::SessionState;
use serde::Serialize;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TmuxError {
    #[error("tmux not found on PATH")]
    NotFound,
    #[error("tmux list-sessions failed")]
    ListFailed,
    #[error("tmux list-sessions output: {0}")]
    Output(String),
}

/// One tmux session from `list-sessions -F`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TmuxSession {
    pub name: String,
    pub path: String,
}

/// Parse `tmux list-sessions -F "#{session_name}\t#{session_path}"` lines.
pub fn parse_list_sessions(stdout: &str) -> Vec<TmuxSession> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (name, path) = match line.split_once('\t') {
            Some((n, p)) => (n, p),
            None => (line, ""),
        };
        out.push(TmuxSession {
            name: name.to_string(),
            path: path.to_string(),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Run `tmux list-sessions` and return parsed sessions (empty if no server).
pub fn list_tmux_sessions() -> Result<Vec<TmuxSession>, TmuxError> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}\t#{session_path}"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TmuxError::NotFound
            } else {
                TmuxError::ListFailed
            }
        })?;

    if !output.status.success() {
        // No server or no sessions — treat as empty list.
        return Ok(Vec::new());
    }

    let text = String::from_utf8(output.stdout).map_err(|e| TmuxError::Output(e.to_string()))?;
    Ok(parse_list_sessions(&text))
}

/// Signals from the session's active pane (tmux subprocess; absent when tmux is down).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSignals {
    pub idle_seconds: u32,
    pub tail: String,
}

/// v0 pane heuristics (hooks deferred per architecture.md).
pub fn classify_pane_heuristics(
    idle_seconds: u32,
    pane_tail: &str,
    idle_seconds_for_needs_input: u32,
) -> SessionState {
    let threshold = idle_seconds_for_needs_input.max(1);
    if pane_tail_looks_like_input(pane_tail) && idle_seconds >= threshold {
        return SessionState::NeedsInput;
    }
    if idle_seconds <= 1 {
        return SessionState::Active;
    }
    if idle_seconds < threshold {
        return SessionState::Active;
    }
    SessionState::Idle
}

fn pane_tail_looks_like_input(tail: &str) -> bool {
    let line = tail
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(str::trim)
        .unwrap_or("");
    if line.is_empty() {
        return false;
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("(y/n)")
        || lower.contains("[y/n]")
        || lower.contains("yes/no")
        || lower.ends_with('?')
        || lower.ends_with(":")
    {
        return true;
    }
    line.ends_with('>') || line.ends_with("›") || line.ends_with("»")
}

pub fn pane_signals_for_session(session_name: &str) -> Option<PaneSignals> {
    let idle = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            session_name,
            "-F",
            "#{pane_active} #{pane_idle_time}",
        ])
        .output()
        .ok()?;
    if !idle.status.success() {
        return None;
    }
    let idle_text = String::from_utf8_lossy(&idle.stdout);
    let idle_seconds = parse_active_pane_idle(&idle_text);

    let capture = Command::new("tmux")
        .args([
            "capture-pane",
            "-p",
            "-t",
            session_name,
            "-S",
            "-30",
        ])
        .output()
        .ok()?;
    if !capture.status.success() {
        return None;
    }
    let tail = String::from_utf8_lossy(&capture.stdout).to_string();
    Some(PaneSignals {
        idle_seconds,
        tail,
    })
}

/// Capture active pane text for a session (negative start = lines from bottom).
pub fn capture_pane_text(session_name: &str, max_lines: i32) -> Result<String, TmuxError> {
    let start = format!("-{}", max_lines.max(1));
    let capture = Command::new("tmux")
        .args([
            "capture-pane",
            "-p",
            "-e",
            "-t",
            session_name,
            "-S",
            &start,
        ])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TmuxError::NotFound
            } else {
                TmuxError::ListFailed
            }
        })?;
    if !capture.status.success() {
        return Err(TmuxError::ListFailed);
    }
    Ok(String::from_utf8_lossy(&capture.stdout).to_string())
}

/// Send a literal line to the session's active pane; optional Enter keypress.
pub fn send_line_to_session(session_name: &str, line: &str, enter: bool) -> Result<(), TmuxError> {
    let send = Command::new("tmux")
        .args(["send-keys", "-t", session_name, "-l", "--", line])
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TmuxError::NotFound
            } else {
                TmuxError::ListFailed
            }
        })?;
    if !send.success() {
        return Err(TmuxError::ListFailed);
    }
    if enter {
        let enter_status = Command::new("tmux")
            .args(["send-keys", "-t", session_name, "Enter"])
            .status()
            .map_err(|_| TmuxError::ListFailed)?;
        if !enter_status.success() {
            return Err(TmuxError::ListFailed);
        }
    }
    Ok(())
}

fn parse_active_pane_idle(list_panes_output: &str) -> u32 {
    for line in list_panes_output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "1" {
            return parts[1].parse().unwrap_or(0);
        }
    }
    list_panes_output
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sessions_tab_separated() {
        let raw = "c-foo\t/home/foo\nc-bar\t/home/bar\n";
        let sessions = parse_list_sessions(raw);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "c-bar");
        assert_eq!(sessions[1].name, "c-foo");
    }

    #[test]
    fn parse_sessions_name_only() {
        let sessions = parse_list_sessions("solo\n");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "solo");
        assert!(sessions[0].path.is_empty());
    }

    #[test]
    fn classify_needs_input_on_prompt() {
        let tail = "some output\nDo you want to continue? (y/n) ";
        assert_eq!(
            classify_pane_heuristics(5, tail, 3),
            SessionState::NeedsInput
        );
    }

    #[test]
    fn classify_active_when_recent() {
        assert_eq!(
            classify_pane_heuristics(0, "working…\n", 3),
            SessionState::Active
        );
    }

    #[test]
    fn classify_idle_when_quiet() {
        assert_eq!(
            classify_pane_heuristics(60, "done.\n", 3),
            SessionState::Idle
        );
    }

    #[test]
    fn prompt_at_cursor_without_idle_is_active() {
        let tail = "user> ";
        assert_eq!(
            classify_pane_heuristics(0, tail, 3),
            SessionState::Active
        );
    }
}
