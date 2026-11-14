//! Fleet sidebar pane (#321): one pane bound to one tmux client, moved with
//! `join-pane` next to the active pane of whichever session that client shows.
//!
//! Spike (PLAN-321): a sidebar per window needs a live process and state sync per
//! window; moving a single pane keeps one live list and one cursor. Every call that
//! switches a view names the client (`-c`), so another attached client is never
//! switched. Limit: that other client loses the pane if it was looking at the window
//! the sidebar leaves.

use std::process::{Command, Stdio};

/// Pane option naming the client a sidebar pane follows (also how a sidebar is found).
pub const SIDEBAR_PANE_OPTION: &str = "@cdamux_sidebar";
pub const DEFAULT_SIDEBAR_WIDTH: u16 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSide {
    Left,
    Right,
}

fn tmux(args: &[&str]) -> Result<String, String> {
    let out = Command::new("tmux")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("tmux: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Session shown by `client`.
pub fn client_session(client: &str) -> Result<String, String> {
    let out = tmux(&["list-clients", "-F", "#{client_name}\t#{session_name}"])?;
    out.lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(name, _)| *name == client)
        .map(|(_, session)| session.to_string())
        .ok_or_else(|| format!("tmux client {client} not found"))
}

/// Name of the client a bind should target when none is passed (`#{client_name}`).
pub fn default_client() -> Result<String, String> {
    tmux(&["display-message", "-p", "#{client_name}"]).and_then(|c| {
        if c.is_empty() {
            Err("no tmux client (pass --client '#{client_name}')".into())
        } else {
            Ok(c)
        }
    })
}

/// Pane id of the sidebar following `client`, if one exists in any session.
pub fn find_sidebar(client: &str) -> Option<String> {
    let fmt = format!("#{{pane_id}}\t#{{{SIDEBAR_PANE_OPTION}}}");
    let out = tmux(&["list-panes", "-a", "-F", &fmt]).ok()?;
    parse_sidebar_pane(&out, client)
}

fn parse_sidebar_pane(list: &str, client: &str) -> Option<String> {
    list.lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(_, owner)| *owner == client)
        .map(|(pane, _)| pane.to_string())
}

fn session_pane(session: &str) -> Result<String, String> {
    tmux(&["display-message", "-p", "-t", &format!("={session}:"), "#{pane_id}"])
}

/// Flags placing a full-height column of `width` cells; `-b` puts it left of the target.
fn column_flags(side: SidebarSide, width: u16) -> Vec<String> {
    let mut flags = vec!["-h".to_string(), "-f".into()];
    if side == SidebarSide::Left {
        flags.push("-b".into());
    }
    flags.extend(["-l".into(), width.to_string()]);
    flags
}

/// Open a sidebar running `command` next to the active pane of `client`'s session.
/// Returns the new pane id (focused).
pub fn spawn_sidebar(
    client: &str,
    side: SidebarSide,
    width: u16,
    command: &[String],
) -> Result<String, String> {
    // Not `display-message -c`: that names the client but resolves the pane from the most
    // recently used session, which is the other client's with two attached.
    let target = session_pane(&client_session(client)?)?;
    let mut args = vec!["split-window".to_string()];
    args.extend(column_flags(side, width));
    args.extend(["-t".into(), target, "-P".into(), "-F".into(), "#{pane_id}".into(), "--".into()]);
    args.extend(command.iter().cloned());
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let pane = tmux(&argv)?;
    tmux(&["set-option", "-p", "-t", &pane, SIDEBAR_PANE_OPTION, client])?;
    Ok(pane)
}

/// Bring the sidebar `pane` to `session` and show that session on `client` only.
/// `focus`: leave the sidebar selected (cursor-driven) instead of the pane it joined.
pub fn follow_session(
    client: &str,
    pane: &str,
    session: &str,
    side: SidebarSide,
    width: u16,
    focus: bool,
) -> Result<(), String> {
    let here = tmux(&["display-message", "-p", "-t", pane, "#{session_name}"])?;
    if here != session {
        let mut args = vec!["join-pane".to_string()];
        args.extend(column_flags(side, width));
        if !focus {
            args.push("-d".into());
        }
        args.extend(["-s".into(), pane.into(), "-t".into(), session_pane(session)?]);
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        tmux(&argv)?;
    }
    if client_session(client)? != session {
        tmux(&["switch-client", "-c", client, "-t", &format!("={session}")])?;
    }
    if focus {
        tmux(&["select-pane", "-t", pane])?;
    }
    Ok(())
}

/// Select the pane the sidebar sits next to (the one that was active before it).
pub fn focus_main_pane(pane: &str) -> Result<(), String> {
    tmux(&["select-pane", "-l", "-t", pane]).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_the_sidebar_of_this_client() {
        let list = "%1\t\n%7\t/dev/ttys001\n%9\t/dev/ttys002";
        assert_eq!(parse_sidebar_pane(list, "/dev/ttys002").as_deref(), Some("%9"));
        assert_eq!(parse_sidebar_pane(list, "/dev/ttys003"), None);
    }

    #[test]
    fn column_flags_place_the_sidebar() {
        assert_eq!(column_flags(SidebarSide::Right, 30), ["-h", "-f", "-l", "30"]);
        assert_eq!(column_flags(SidebarSide::Left, 30), ["-h", "-f", "-b", "-l", "30"]);
    }
}
