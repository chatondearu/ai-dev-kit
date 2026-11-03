//! Push keys/text to a tmux session pane without attaching (shared by CLI and future daemon HTTP).

use crate::tmux::{send_line_to_session, TmuxError};
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SendError {
    #[error("tmux not found on PATH")]
    NotFound,
    #[error("no tmux session named '{0}'")]
    SessionNotFound(String),
    #[error("tmux send-keys failed")]
    SendFailed,
}

/// `has-session` target that matches the exact name (plain `-t` prefix-matches).
fn exact_session_target(session: &str) -> String {
    format!("={session}")
}

pub fn tmux_has_session(session: &str) -> Result<bool, SendError> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", &exact_session_target(session)])
        // "can't find session" would land on top of the TUI; the exit code is the answer.
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SendError::NotFound
            } else {
                SendError::SendFailed
            }
        })?;
    Ok(status.success())
}

/// Send literal text (and optional Enter) to a session's active pane without attaching.
pub fn send_to_session(session: &str, text: &str, enter: bool) -> Result<(), SendError> {
    if text.is_empty() && !enter {
        return Ok(());
    }
    if !tmux_has_session(session)? {
        return Err(SendError::SessionNotFound(session.to_string()));
    }
    send_line_to_session(session, text, enter).map_err(|e| match e {
        TmuxError::NotFound => SendError::NotFound,
        _ => SendError::SendFailed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_session_target_is_exact_match() {
        assert_eq!(exact_session_target("c-app"), "=c-app");
    }

    #[test]
    fn missing_session_error_text() {
        let err = SendError::SessionNotFound("nope".into());
        assert_eq!(err.to_string(), "no tmux session named 'nope'");
    }
}
