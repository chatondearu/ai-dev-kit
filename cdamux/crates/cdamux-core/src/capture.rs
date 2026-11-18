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
    let text = tmux_read(
        &["capture-pane", "-p", "-J", "-t", &pane, "-S", &format!("-{lines}")],
        "capture-pane",
    )?;
    Ok(PaneCapture {
        v: 1,
        session: session.to_string(),
        pane,
        lines,
        text,
    })
}
