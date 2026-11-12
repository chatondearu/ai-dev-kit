//! Kill, rename, fork naming, attach helpers.

use crate::send::{spawn_err, tmux_has_session, SendError};
use crate::tmux::{list_tmux_sessions, HOME_SESSION_NAME};
use std::process::{Command, Stdio};

/// Run `tmux <args>`; `SendFailed` on a non-zero exit.
fn tmux_ok(args: &[&str]) -> Result<(), SendError> {
    let status = Command::new("tmux").args(args).status().map_err(spawn_err)?;
    if status.success() {
        Ok(())
    } else {
        Err(SendError::SendFailed)
    }
}

/// True when `name` is the cdamux home / dashboard session.
pub fn is_home_session(name: &str) -> bool {
    name == HOME_SESSION_NAME
}

/// Args for `tmux new-session -A -s cdamux cdamux` (outside-tmux dashboard entry).
pub fn home_session_new_argv() -> [&'static str; 5] {
    ["new-session", "-A", "-s", HOME_SESSION_NAME, "cdamux"]
}

/// Create the home session detached if missing (runs `cdamux` as the pane command).
/// An existing home pane running a plain shell (tmux-resurrect / continuum restore it
/// with the default shell over the stale screen) is respawned with `cdamux`.
pub fn ensure_home_session() -> Result<(), SendError> {
    if !tmux_has_session(HOME_SESSION_NAME)? {
        return tmux_ok(&["new-session", "-d", "-s", HOME_SESSION_NAME, "cdamux"]);
    }
    let target = format!("={HOME_SESSION_NAME}:");
    if pane_current_command(&target).is_some_and(|c| is_shell_command(&c)) {
        // Best effort: a failed repair must not block the kill that needs a home session.
        let _ = tmux_ok(&["respawn-pane", "-k", "-t", &target, "cdamux"]);
    }
    Ok(())
}

fn pane_current_command(target: &str) -> Option<String> {
    let out = Command::new("tmux")
        .args(["display-message", "-p", "-t", target, "#{pane_current_command}"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Login shells show as `-fish`, `-bash`, …
fn is_shell_command(cmd: &str) -> bool {
    matches!(
        cmd.trim_start_matches('-'),
        "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" | "mksh" | "tcsh" | "csh" | "nu"
    )
}

/// Current client session name when inside tmux (`None` outside or on failure).
pub fn current_tmux_session() -> Option<String> {
    if std::env::var_os("TMUX").is_none() {
        return None;
    }
    let out = Command::new("tmux")
        .args(["display-message", "-p", "#{session_name}"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Ensure home before kill when this client is on `killing` and no other visible
/// (non-home) session would remain — so the client has a session to land on.
pub fn should_ensure_home_before_kill(
    current: Option<&str>,
    killing: &str,
    visible_non_home: &[&str],
) -> bool {
    if is_home_session(killing) {
        return false;
    }
    current == Some(killing) && !visible_non_home.iter().any(|n| *n != killing)
}

/// Read global `detach-on-destroy` (`None` if tmux missing / option unset).
fn detach_on_destroy_get() -> Option<String> {
    let out = Command::new("tmux")
        .args(["show-options", "-gv", "detach-on-destroy"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!v.is_empty()).then_some(v)
}

fn detach_on_destroy_set(value: &str) -> Result<(), SendError> {
    tmux_ok(&["set-option", "-g", "detach-on-destroy", value])
}

/// Switch the current client to `session` without setting `@cdamux_return`
/// (used before kill so pane-died does not reopen a popup).
fn switch_client_quiet(session: &str) -> Result<(), SendError> {
    tmux_ok(&["switch-client", "-t", &format!("={session}")])
}

pub fn kill_tmux_session(name: &str) -> Result<(), SendError> {
    if !tmux_has_session(name)? {
        return Err(SendError::SessionNotFound(name.to_string()));
    }
    let current = current_tmux_session();
    let visible: Vec<String> = list_tmux_sessions()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.name)
        .collect();
    let visible_refs: Vec<&str> = visible.iter().map(String::as_str).collect();
    // Leave the session before destroying it so the client is not detached.
    if current.as_deref() == Some(name) && !is_home_session(name) {
        ensure_home_session()?;
        let _ = switch_client_quiet(HOME_SESSION_NAME);
    } else if should_ensure_home_before_kill(current.as_deref(), name, &visible_refs) {
        ensure_home_session()?;
    }
    // tmux 3.7.x: `detach-on-destroy next|previous` makes any kill-session exit
    // the whole server. Flip to `on` and leave it (do not restore — a later
    // bare kill-session would take the server down again).
    let previous = detach_on_destroy_get();
    if previous.as_deref() == Some("next") || previous.as_deref() == Some("previous") {
        let _ = detach_on_destroy_set("on");
    }
    tmux_ok(&["kill-session", "-t", &format!("={name}")])
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
    tmux_ok(&["rename-session", "-t", old, new])
}

/// `base`, `base-2`, `base-3`, … first name without a live session or a restore record
/// (reusing a dormant session's name would overwrite its record).
pub fn unique_session_name(base: &str) -> String {
    unique_session_name_with(base, |name| {
        tmux_has_session(name).unwrap_or(false) || crate::session_registry::load(name).is_some()
    })
}

/// Same numbering as [`unique_session_name`], for unit tests without tmux.
pub(crate) fn unique_session_name_with(base: &str, exists: impl Fn(&str) -> bool) -> String {
    if exists(base) {
        for n in 2..1000 {
            let candidate = format!("{base}-{n}");
            if !exists(&candidate) {
                return candidate;
            }
        }
    }
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_name_skips_occupied_base_and_suffixes() {
        let occupied = |name: &str| matches!(name, "c-proj" | "c-proj-2");
        assert_eq!(
            unique_session_name_with("c-proj", occupied),
            "c-proj-3"
        );
        assert_eq!(unique_session_name_with("c-other", occupied), "c-other");
    }

    #[test]
    fn home_session_new_argv_is_attach_or_create() {
        assert_eq!(
            home_session_new_argv(),
            ["new-session", "-A", "-s", "cdamux", "cdamux"]
        );
        assert!(is_home_session("cdamux"));
        assert!(!is_home_session("c-proj"));
    }

    #[test]
    fn shell_detection_strips_login_dash() {
        assert!(is_shell_command("fish") && is_shell_command("-zsh") && is_shell_command("nu"));
        assert!(!is_shell_command("cdamux") && !is_shell_command("nvim") && !is_shell_command(""));
    }

    #[test]
    fn ensure_home_before_kill_only_for_last_attached() {
        assert!(should_ensure_home_before_kill(
            Some("work"),
            "work",
            &["work"]
        ));
        assert!(!should_ensure_home_before_kill(
            Some("work"),
            "work",
            &["work", "other"]
        ));
        assert!(!should_ensure_home_before_kill(
            Some("other"),
            "work",
            &["work"]
        ));
        assert!(!should_ensure_home_before_kill(
            Some("cdamux"),
            "cdamux",
            &[]
        ));
    }
}

/// When the agent pane exits, tmux pane-died hook can reopen cdamux (see aitools tmux template).
pub fn mark_session_cdamux_return(session: &str) -> Result<(), SendError> {
    if !tmux_has_session(session)? {
        return Err(SendError::SessionNotFound(session.to_string()));
    }
    tmux_ok(&["set-option", "-t", session, "@cdamux_return", "1"])
}

pub fn switch_client_to_session(session: &str) -> Result<(), SendError> {
    mark_session_cdamux_return(session)?;
    tmux_ok(&["switch-client", "-t", session])
}
