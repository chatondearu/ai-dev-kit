//! Push keys/text to a tmux session pane without attaching (shared by CLI and future daemon HTTP).

use std::process::Command;
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

/// Build `tmux send-keys` argv for unit tests and daemon reuse.
///
/// Targeting: `-t <session>` resolves to that session's **active window, active pane**
/// (tmux default when the target is a session name).
pub fn build_send_keys_argv(session: &str, text: &str, enter: bool) -> Vec<String> {
    let mut argv = vec![
        "send-keys".to_string(),
        "-l".to_string(),
        "-t".to_string(),
        session.to_string(),
    ];
    if !text.is_empty() {
        argv.push(text.to_string());
    }
    if enter {
        argv.push("Enter".to_string());
    }
    argv
}

pub fn tmux_has_session(session: &str) -> Result<bool, SendError> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", session])
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

/// Send literal text (and optional Enter) to a session without attaching.
pub fn send_to_session(session: &str, text: &str, enter: bool) -> Result<(), SendError> {
    if text.is_empty() && !enter {
        return Ok(());
    }
    match tmux_has_session(session)? {
        true => {}
        false => return Err(SendError::SessionNotFound(session.to_string())),
    }

    let argv = build_send_keys_argv(session, text, enter);
    let mut cmd = Command::new("tmux");
    cmd.args(&argv);
    let status = cmd.status().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SendError::NotFound
        } else {
            SendError::SendFailed
        }
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(SendError::SendFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_literal_text_no_enter() {
        let argv = build_send_keys_argv("c-foo", "hello world", false);
        assert_eq!(
            argv,
            vec![
                "send-keys",
                "-l",
                "-t",
                "c-foo",
                "hello world",
            ]
        );
    }

    #[test]
    fn argv_with_enter() {
        let argv = build_send_keys_argv("c-bar", "y", true);
        assert_eq!(
            argv,
            vec!["send-keys", "-l", "-t", "c-bar", "y", "Enter"]
        );
    }

    #[test]
    fn argv_enter_only() {
        let argv = build_send_keys_argv("s", "", true);
        assert_eq!(argv, vec!["send-keys", "-l", "-t", "s", "Enter"]);
    }

    #[test]
    fn missing_session_error_text() {
        let err = SendError::SessionNotFound("nope".into());
        assert_eq!(err.to_string(), "no tmux session named 'nope'");
    }
}
