//! Read-only pane capture for `cdamux capture` (#388).
//! Only `has-session`, `list-sessions`, `display-message -p` and `capture-pane` run here:
//! no input path (`send-keys`, `paste-buffer`, …) by design.

use crate::send::{tmux_has_session, SendError};
use crate::tmux::{list_tmux_sessions, pane_target, TmuxError};
use serde::Serialize;
use std::process::{Command, Stdio};
use thiserror::Error;

/// Upper bound for `--lines` (clap rejects larger values with exit 2).
pub const CAPTURE_MAX_LINES: u16 = 500;

#[derive(Debug, Error)]
pub enum CaptureError {
    /// Not listed by cdamux (or, with `any`, no such tmux session).
    #[error("unknown session '{0}'")]
    Unknown(String),
    #[error("tmux not found on PATH")]
    NotFound,
    #[error("tmux {0} failed")]
    Tmux(&'static str),
}

/// `--json` shape: `{"v":1,"session","pane","lines","text"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaneCapture {
    pub v: u8,
    pub session: String,
    /// tmux pane id of the captured (active) pane, e.g. `%3`.
    pub pane: String,
    /// Lines actually returned (≤ the requested count).
    pub lines: u16,
    pub text: String,
}

fn tmux_read(args: &[&str], what: &'static str) -> Result<String, CaptureError> {
    let out = Command::new("tmux")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CaptureError::NotFound
            } else {
                CaptureError::Tmux(what)
            }
        })?;
    if !out.status.success() {
        return Err(CaptureError::Tmux(what));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Plain text (`capture-pane -p -J`) of the last `lines` lines of `session`'s active pane.
/// Without `any`, only sessions `cdamux list` shows are accepted.
pub fn capture_session(session: &str, lines: u16, any: bool) -> Result<PaneCapture, CaptureError> {
    let known = if any {
        tmux_has_session(session).map_err(|e| match e {
            SendError::NotFound => CaptureError::NotFound,
            _ => CaptureError::Tmux("has-session"),
        })?
    } else {
        list_tmux_sessions()
            .map_err(|e| match e {
                TmuxError::NotFound => CaptureError::NotFound,
                _ => CaptureError::Tmux("list-sessions"),
            })?
            .iter()
            .any(|s| s.name == session)
    };
    if !known {
        return Err(CaptureError::Unknown(session.to_string()));
    }
    let lines = lines.clamp(1, CAPTURE_MAX_LINES);
    let target = pane_target(session);
    let pane = tmux_read(&["display-message", "-p", "-t", &target, "#{pane_id}"], "display-message")?
        .trim()
        .to_string();
    // Capture by pane id so a window switch between the two calls cannot change the pane.
    // `-S -N` returns N history lines plus the whole visible pane: trimmed below.
    let raw = tmux_read(
        &["capture-pane", "-p", "-J", "-t", &pane, "-S", &format!("-{lines}")],
        "capture-pane",
    )?;
    let (text, lines) = last_lines(&raw, lines);
    Ok(PaneCapture {
        v: 1,
        session: session.to_string(),
        pane,
        lines,
        text,
    })
}

/// Last `n` lines of `text` after dropping trailing blank rows (unused pane rows),
/// newline-terminated, and how many were kept.
fn last_lines(text: &str, n: u16) -> (String, u16) {
    let all: Vec<&str> = text.lines().collect();
    let end = all
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let kept = &all[end.saturating_sub(n as usize)..end];
    let mut out = kept.join("\n");
    if !kept.is_empty() {
        out.push('\n');
    }
    (out, kept.len() as u16)
}

#[cfg(test)]
mod tests {
    use super::last_lines;

    #[test]
    fn last_lines_keeps_tail_and_drops_blank_rows() {
        let raw = "h1\nh2\na\n\nb\n\n   \n\n";
        assert_eq!(last_lines(raw, 3), ("a\n\nb\n".into(), 3));
        assert_eq!(last_lines(raw, 1), ("b\n".into(), 1));
        assert_eq!(last_lines(raw, 500), ("h1\nh2\na\n\nb\n".into(), 5));
        assert_eq!(last_lines("\n\n", 5), (String::new(), 0));
    }
}
