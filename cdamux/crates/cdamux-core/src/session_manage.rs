//! Kill, rename, fork naming, attach helpers.

use crate::send::{tmux_has_session, SendError};
use std::process::Command;

pub fn kill_tmux_session(name: &str) -> Result<(), SendError> {
    if !tmux_has_session(name)? {
        return Err(SendError::SessionNotFound(name.to_string()));
    }
    let status = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .status()
        .map_err(|e| {
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

pub fn rename_tmux_session(old: &str, new: &str) -> Result<(), SendError> {
    if new.trim().is_empty() {
        return Err(SendError::SendFailed);
    }
    if !tmux_has_session(old)? {
        return Err(SendError::SessionNotFound(old.to_string()));
    }
    if tmux_has_session(new)? {
        return Err(SendError::SendFailed);
    }
    let status = Command::new("tmux")
        .args(["rename-session", "-t", old, new])
        .status()
        .map_err(|e| {
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

/// `base`, `base-2`, `base-3`, … first name without an existing session.
pub fn unique_session_name(base: &str) -> String {
    if tmux_has_session(base).unwrap_or(false) {
        for n in 2..1000 {
            let candidate = format!("{base}-{n}");
            if !tmux_has_session(&candidate).unwrap_or(false) {
                return candidate;
            }
        }
    }
    base.to_string()
}

/// When the agent pane exits, tmux pane-died hook can reopen cdamux (see aitools tmux template).
pub fn mark_session_cdamux_return(session: &str) -> Result<(), SendError> {
    if !tmux_has_session(session)? {
        return Err(SendError::SessionNotFound(session.to_string()));
    }
    let status = Command::new("tmux")
        .args(["set-option", "-t", session, "@cdamux_return", "1"])
        .status()
        .map_err(|_| SendError::SendFailed)?;
    if status.success() {
        Ok(())
    } else {
        Err(SendError::SendFailed)
    }
}

pub fn switch_client_to_session(session: &str) -> Result<(), SendError> {
    mark_session_cdamux_return(session)?;
    let status = Command::new("tmux")
        .args(["switch-client", "-t", session])
        .status()
        .map_err(|e| {
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
