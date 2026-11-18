//! Push keys/text to a tmux session pane without attaching (shared by CLI and future daemon HTTP).

use crate::tmux::{
    is_forbidden_control, pane_target, send_input_to_target, send_line_to_session, send_line_to_session_preclear, TmuxError,
};
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
    #[error("key '{0}' is not allowed")]
    KeyNotAllowed(String),
    #[error("no pane '{0}' in that session")]
    PaneNotFound(String),
    #[error("text contains a control character (only \\n and \\t are allowed; use --key)")]
    ControlInText,
}

/// Named keys `cdamux send --key` may press, spelled as tmux `send-keys` key names.
/// Anything else is refused: a raw tmux key string is never passed through.
pub const SEND_KEYS: &[&str] = &[
    "Enter", "Escape", "Tab", "BTab", "Up", "Down", "Left", "Right", "PageUp", "PageDown",
    "Home", "End", "BSpace", "C-c", "C-d", "C-z", "C-l", "C-r",
];

/// What [`send_input`] delivered: the pane id it wrote to, text bytes, keys pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputOutcome {
    pub pane: String,
    pub sent_bytes: usize,
    pub keys: Vec<String>,
}

/// `has-session` target that matches the exact name (plain `-t` prefix-matches).
fn exact_session_target(session: &str) -> String {
    format!("={session}")
}

/// Error for a failed `tmux` spawn: `NotFound` when tmux is not on PATH.
pub(crate) fn spawn_err(e: std::io::Error) -> SendError {
    if e.kind() == std::io::ErrorKind::NotFound {
        SendError::NotFound
    } else {
        SendError::SendFailed
    }
}

pub fn tmux_has_session(session: &str) -> Result<bool, SendError> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", &exact_session_target(session)])
        // "can't find session" would land on top of the TUI; the exit code is the answer.
        .stderr(Stdio::null())
        .status()
        .map_err(spawn_err)?;
    Ok(status.success())
}

/// Send literal text (and optional Enter) to a session's active pane without attaching.
pub fn send_to_session(session: &str, text: &str, enter: bool) -> Result<(), SendError> {
    send_to_session_ex(session, text, enter, false)
}

/// Like [`send_to_session`], but `C-u` first so leftovers do not accumulate in the TUI
/// composer (Cursor orch idle nudge path).
pub(crate) fn send_to_session_preclear(
    session: &str,
    text: &str,
    enter: bool,
) -> Result<(), SendError> {
    send_to_session_ex(session, text, enter, true)
}

fn send_to_session_ex(
    session: &str,
    text: &str,
    enter: bool,
    clear_line: bool,
) -> Result<(), SendError> {
    if text.is_empty() && !enter {
        return Ok(());
    }
    if !tmux_has_session(session)? {
        return Err(SendError::SessionNotFound(session.to_string()));
    }
    let send = if clear_line {
        send_line_to_session_preclear(session, text, enter)
    } else {
        send_line_to_session(session, text, enter)
    };
    send.map_err(|e| match e {
        TmuxError::NotFound => SendError::NotFound,
        _ => SendError::SendFailed,
    })
}

/// Map each requested key to its [`SEND_KEYS`] entry; the first unknown name is an error.
fn allowed_keys(keys: &[String]) -> Result<Vec<&'static str>, SendError> {
    keys.iter()
        .map(|k| {
            SEND_KEYS
                .iter()
                .copied()
                .find(|a| a == k)
                .ok_or_else(|| SendError::KeyNotAllowed(k.clone()))
        })
        .collect()
}

fn tmux_stdout(args: &[&str]) -> Result<String, SendError> {
    let out = Command::new("tmux")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .map_err(spawn_err)?;
    if !out.status.success() {
        return Err(SendError::SendFailed);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Resolve the pane to write to: `pane` must be one of the session's pane ids; default is
/// the active pane of the active window.
fn resolve_pane(session: &str, pane: Option<&str>) -> Result<String, SendError> {
    let Some(pane) = pane else {
        let id = tmux_stdout(&["display-message", "-p", "-t", &pane_target(session), "#{pane_id}"])?;
        return Ok(id.trim().to_string());
    };
    let panes = tmux_stdout(&["list-panes", "-s", "-t", &exact_session_target(session), "-F", "#{pane_id}"])?;
    if panes.lines().any(|p| p == pane) {
        Ok(pane.to_string())
    } else {
        Err(SendError::PaneNotFound(pane.to_string()))
    }
}

/// Send literal `text`, then allow-listed `keys`, then optional Enter to one pane of `session`.
///
/// Keys are checked against [`SEND_KEYS`] before tmux is touched. `text` keeps the
/// [`send_to_session`] paste path; control characters other than `\n`/`\t` are refused.
pub fn send_input(
    session: &str,
    pane: Option<&str>,
    text: &str,
    keys: &[String],
    enter: bool,
) -> Result<InputOutcome, SendError> {
    let keys = allowed_keys(keys)?;
    if text.chars().any(is_forbidden_control) {
        return Err(SendError::ControlInText);
    }
    if !tmux_has_session(session)? {
        return Err(SendError::SessionNotFound(session.to_string()));
    }
    let pane = resolve_pane(session, pane)?;
    if !text.is_empty() || !keys.is_empty() || enter {
        send_input_to_target(&pane, text, &keys, enter).map_err(|e| match e {
            TmuxError::NotFound => SendError::NotFound,
            _ => SendError::SendFailed,
        })?;
    }
    Ok(InputOutcome {
        pane,
        sent_bytes: text.len(),
        keys: keys.iter().map(|k| k.to_string()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_outside_allow_list_are_refused() {
        let ok = allowed_keys(&["C-c".into(), "Escape".into(), "PageDown".into()]).unwrap();
        assert_eq!(ok, vec!["C-c", "Escape", "PageDown"]);
        for bad in ["Foo", "C-c; kill-server", "c-c", "C-x", "", "-l"] {
            assert_eq!(
                allowed_keys(&["Enter".into(), bad.into()]),
                Err(SendError::KeyNotAllowed(bad.into()))
            );
        }
    }

    #[test]
    fn control_chars_in_text_fail_before_tmux() {
        for text in ["echo hi\r", "a\x03", "\x1b[A"] {
            let err = send_input("=no such session=", None, text, &[], false).unwrap_err();
            assert_eq!(err, SendError::ControlInText, "{text:?}");
        }
    }

    #[test]
    fn bad_key_fails_before_tmux() {
        // A session that cannot exist: KeyNotAllowed wins over SessionNotFound / NotFound.
        let err = send_input("=no such session=", None, "", &["Foo".into()], false).unwrap_err();
        assert_eq!(err, SendError::KeyNotAllowed("Foo".into()));
    }

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
