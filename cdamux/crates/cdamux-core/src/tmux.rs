//! Parse and list tmux sessions (CLI subprocess v0).

use crate::session::SessionState;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

/// Dedicated full-screen dashboard session; hidden from Fleet / Sessions lists.
pub const HOME_SESSION_NAME: &str = "cdamux";

#[derive(Debug, Error)]
pub enum TmuxError {
    #[error("tmux not found on PATH")]
    NotFound,
    #[error("tmux list-sessions failed")]
    ListFailed,
    #[error("tmux list-sessions output: {0}")]
    Output(String),
}

/// One tmux session from `list-sessions -F`, with its session-link user options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TmuxSession {
    pub name: String,
    pub path: String,
    /// `@cdamux_parent`: name of the session this one was spawned from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// `@cdamux_run`: orch run id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// `@cdamux_role`: agent | worker | subagent | shell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Session user options resolve in `list-sessions` formats: one call reads every link.
const LIST_SESSIONS_FORMAT: &str =
    "#{session_name}\t#{session_path}\t#{@cdamux_parent}\t#{@cdamux_run}\t#{@cdamux_role}";

/// Parse [`LIST_SESSIONS_FORMAT`] lines (trailing fields optional).
pub fn parse_list_sessions(stdout: &str) -> Vec<TmuxSession> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let mut f = line.split('\t');
        out.push(TmuxSession {
            name: f.next().unwrap_or_default().to_string(),
            path: f.next().unwrap_or_default().to_string(),
            parent: f.next().and_then(non_empty),
            run: f.next().and_then(non_empty),
            role: f.next().and_then(non_empty),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Run `tmux list-sessions` and return parsed sessions (empty if no server).
pub fn list_tmux_sessions() -> Result<Vec<TmuxSession>, TmuxError> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", LIST_SESSIONS_FORMAT])
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
    Ok(parse_list_sessions(&text)
        .into_iter()
        .filter(|s| s.name != HOME_SESSION_NAME)
        .collect())
}

/// Per-session snapshot from one `tmux list-panes -a` call (idle + hook user options).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionPaneInfo {
    /// Active pane idle; `None` when `pane_idle_time` is empty/unsupported (do not treat as 0 → Active).
    pub idle_seconds: Option<u32>,
    /// Raw `@cdamux_state` set by hooks, if any.
    pub hook_state: Option<String>,
    pub cdamux_agent: Option<String>,
    pub ccmux_agent: Option<String>,
}

/// Session user options resolve through pane formats, so one `-a` call covers every session.
const LIST_PANES_ALL_FORMAT: &str =
    "#{session_name}\t#{pane_active}\t#{pane_idle_time}\t#{@cdamux_state}\t#{@cdamux_agent}\t#{@ccmux_agent}";

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Parse [`LIST_PANES_ALL_FORMAT`] lines into a map keyed by session name.
/// Idle comes from the active pane, else the session's first pane.
pub fn parse_list_panes_all(stdout: &str) -> HashMap<String, SessionPaneInfo> {
    let mut out: HashMap<String, SessionPaneInfo> = HashMap::new();
    for line in stdout.lines() {
        let mut f = line.split('\t');
        let (Some(name), Some(active), Some(idle)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let is_new = !out.contains_key(name);
        let info = out.entry(name.to_string()).or_default();
        if is_new || active == "1" {
            info.idle_seconds = idle.trim().parse().ok();
        }
        info.hook_state = f.next().and_then(non_empty);
        info.cdamux_agent = f.next().and_then(non_empty);
        info.ccmux_agent = f.next().and_then(non_empty);
    }
    out
}

/// One tmux spawn for all sessions (empty map when tmux is down or has no server).
pub fn list_panes_all() -> HashMap<String, SessionPaneInfo> {
    let out = Command::new("tmux")
        .args(["list-panes", "-a", "-F", LIST_PANES_ALL_FORMAT])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_list_panes_all(&String::from_utf8_lossy(&o.stdout)),
        _ => HashMap::new(),
    }
}

/// `display-message -p` format for a user option name (`@foo` → `#{@foo}`).
/// Passing `@foo` alone prints the literal `@foo`, not the option value.
pub(crate) fn user_option_display_format(option: &str) -> String {
    if option.starts_with("#{") {
        option.to_string()
    } else if let Some(name) = option.strip_prefix('@') {
        format!("#{{@{name}}}")
    } else {
        option.to_string()
    }
}

/// Pane target for a session name: `=` forces an exact session match and the trailing
/// `:` stops tmux from resolving the bare name as a window in the current session.
pub(crate) fn pane_target(session_name: &str) -> String {
    format!("={session_name}:")
}

/// Read a tmux user option (e.g. `@cdamux_state`) for a session.
pub fn tmux_user_option(session_name: &str, option: &str) -> Option<String> {
    let format = user_option_display_format(option);
    let out = Command::new("tmux")
        .args(["display-message", "-p", "-t", &pane_target(session_name), &format])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Pane heuristics; prefer `@cdamux_state` when hooks set it (see architecture.md).
pub fn classify_pane_heuristics(
    idle_seconds: Option<u32>,
    pane_tail: &str,
    idle_seconds_for_needs_input: u32,
) -> SessionState {
    // shortcut: empty pane_idle_time → Idle (fail closed); upgrade to pane_last_activity if needed
    let Some(idle_seconds) = idle_seconds else {
        return SessionState::Idle;
    };
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

/// Plain (no escapes) tail of the active pane for the prompt heuristics.
pub fn pane_tail(session_name: &str) -> Option<String> {
    let capture = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", &pane_target(session_name), "-S", "-30"])
        .output()
        .ok()?;
    if !capture.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&capture.stdout).to_string())
}

/// Args for `tmux capture-pane` preview (no `-E -1`: empty history_size panes otherwise blank).
/// `escapes` keeps ANSI colors (`-e`) for the TUI; plain text for API clients.
fn capture_pane_args(session_name: &str, max_lines: i32, escapes: bool) -> Vec<String> {
    let mut args: Vec<String> = vec!["capture-pane".into(), "-p".into()];
    if escapes {
        args.push("-e".into());
    }
    args.extend(["-t".into(), pane_target(session_name), "-S".into(), format!("-{}", max_lines.max(1))]);
    args
}

/// Capture active pane text for a session (negative start = lines from bottom).
pub fn capture_pane_text(session_name: &str, max_lines: i32, escapes: bool) -> Result<String, TmuxError> {
    let args = capture_pane_args(session_name, max_lines, escapes);
    let capture = Command::new("tmux")
        .args(&args)
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

/// tmux argv per invocation needed to type `line` (and optionally submit it).
///
/// `-l` disables key-name lookup for every arg, so `Enter` must be a separate call.
/// Multi-line text goes through a bracketed paste (`load-buffer -` reads stdin) so the
/// receiving shell or agent does not execute each line as it arrives.
fn send_line_argvs(session_name: &str, line: &str, enter: bool) -> Vec<Vec<String>> {
    let target = pane_target(session_name);
    let argv = |args: &[&str]| -> Vec<String> { args.iter().map(|s| s.to_string()).collect() };
    let mut out = Vec::new();
    if line.contains('\n') {
        out.push(argv(&["load-buffer", "-b", PASTE_BUFFER, "-"]));
        out.push(argv(&["paste-buffer", "-p", "-d", "-b", PASTE_BUFFER, "-t", &target]));
    } else if !line.is_empty() {
        out.push(argv(&["send-keys", "-t", &target, "-l", "--", line]));
    }
    if enter {
        out.push(argv(&["send-keys", "-t", &target, "Enter"]));
    }
    out
}

const PASTE_BUFFER: &str = "cdamux-send";

/// Send a literal line to the session's active pane; optional Enter keypress.
pub fn send_line_to_session(session_name: &str, line: &str, enter: bool) -> Result<(), TmuxError> {
    let map_err = |e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TmuxError::NotFound
        } else {
            TmuxError::ListFailed
        }
    };
    for argv in send_line_argvs(session_name, line, enter) {
        let mut cmd = Command::new("tmux");
        cmd.args(&argv);
        let ok = if argv[0] == "load-buffer" {
            let mut child = cmd.stdin(Stdio::piped()).spawn().map_err(map_err)?;
            // Dropping stdin after the write closes the pipe so load-buffer sees EOF.
            let wrote = child.stdin.take().map(|mut i| i.write_all(line.as_bytes()).is_ok());
            child.wait().map_err(map_err)?.success() && wrote == Some(true)
        } else {
            cmd.status().map_err(map_err)?.success()
        };
        if !ok {
            return Err(TmuxError::ListFailed);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_line_enter_is_separate_key_invocation() {
        assert_eq!(
            send_line_argvs("c-bar", "echo hi", true),
            vec![
                vec!["send-keys", "-t", "=c-bar:", "-l", "--", "echo hi"],
                vec!["send-keys", "-t", "=c-bar:", "Enter"],
            ]
        );
        assert_eq!(send_line_argvs("c-bar", "y", false).len(), 1);
        assert_eq!(send_line_argvs("c-bar", "", true).len(), 1); // Enter only
    }

    #[test]
    fn multiline_send_uses_bracketed_paste() {
        assert_eq!(
            send_line_argvs("c-bar", "a\nb", true),
            vec![
                vec!["load-buffer", "-b", "cdamux-send", "-"],
                vec!["paste-buffer", "-p", "-d", "-b", "cdamux-send", "-t", "=c-bar:"],
                vec!["send-keys", "-t", "=c-bar:", "Enter"],
            ]
        );
        assert_eq!(send_line_argvs("c-bar", "a\nb", false).len(), 2);
    }

    #[test]
    fn parse_sessions_tab_separated() {
        let raw = "c-foo\t/home/foo\nc-bar\t/home/bar\n";
        let sessions = parse_list_sessions(raw);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "c-bar");
        assert_eq!(sessions[1].name, "c-foo");
    }

    #[test]
    fn home_session_name_is_cdamux() {
        assert_eq!(HOME_SESSION_NAME, "cdamux");
        let raw = "cdamux\t/\nc-foo\t/home/foo\n";
        let visible: Vec<_> = parse_list_sessions(raw)
            .into_iter()
            .filter(|s| s.name != HOME_SESSION_NAME)
            .map(|s| s.name)
            .collect();
        assert_eq!(visible, vec!["c-foo"]);
    }

    #[test]
    fn parse_sessions_reads_link_options() {
        let raw = "c-fork\t/p\tc-main\trun-1\tagent\nc-main\t/p\t\t\t\n";
        let sessions = parse_list_sessions(raw);
        assert_eq!(sessions[0].name, "c-fork");
        assert_eq!(sessions[0].parent.as_deref(), Some("c-main"));
        assert_eq!(sessions[0].run.as_deref(), Some("run-1"));
        assert_eq!(sessions[0].role.as_deref(), Some("agent"));
        assert_eq!(sessions[1].parent, None);
        assert_eq!(sessions[1].role, None);
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
            classify_pane_heuristics(Some(5), tail, 3),
            SessionState::NeedsInput
        );
    }

    #[test]
    fn classify_active_when_recent() {
        assert_eq!(
            classify_pane_heuristics(Some(0), "working…\n", 3),
            SessionState::Active
        );
    }

    #[test]
    fn classify_idle_when_quiet() {
        assert_eq!(
            classify_pane_heuristics(Some(60), "done.\n", 3),
            SessionState::Idle
        );
    }

    #[test]
    fn prompt_at_cursor_without_idle_is_active() {
        let tail = "user> ";
        assert_eq!(
            classify_pane_heuristics(Some(0), tail, 3),
            SessionState::Active
        );
    }

    #[test]
    fn parse_list_panes_all_keys_by_session() {
        // Active pane wins for idle; blank idle → None; blank options → None.
        let raw = "a\t0\t99\t\t\t\nb\t1\t42\twaiting\tclaude\tcursor\nb\t0\t7\twaiting\tclaude\tcursor\na\t1\t\t\t\tpi\n";
        let map = parse_list_panes_all(raw);
        assert_eq!(map.len(), 2);
        let a = &map["a"];
        assert_eq!(a.idle_seconds, None);
        assert_eq!(a.hook_state, None);
        assert_eq!(a.cdamux_agent, None);
        assert_eq!(a.ccmux_agent.as_deref(), Some("pi"));
        let b = &map["b"];
        assert_eq!(b.idle_seconds, Some(42));
        assert_eq!(b.hook_state.as_deref(), Some("waiting"));
        assert_eq!(b.cdamux_agent.as_deref(), Some("claude"));
        // No active pane → first pane's idle.
        let first = parse_list_panes_all("c\t0\t5\t\t\t\nc\t0\t9\t\t\t\n");
        assert_eq!(first["c"].idle_seconds, Some(5));
        assert!(parse_list_panes_all("").is_empty());
    }

    #[test]
    fn missing_idle_classifies_idle_not_active() {
        assert_eq!(
            classify_pane_heuristics(None, "working…\n", 3),
            SessionState::Idle
        );
    }

    #[test]
    fn capture_pane_args_omit_end_history_line() {
        let args = capture_pane_args("c-claude", 40, true);
        assert!(args.contains(&"-e".into()));
        assert!(!capture_pane_args("c-claude", 40, false).contains(&"-e".into()));
        assert!(!args.iter().any(|a| a == "-E"));
        assert!(!args.windows(2).any(|w| w[0] == "-E" && w[1] == "-1"));
        assert!(args.contains(&"-S".into()));
        assert!(args.contains(&"-40".into()));
    }

    #[test]
    fn user_option_format_wraps_at_name() {
        assert_eq!(
            user_option_display_format("@cdamux_state"),
            "#{@cdamux_state}"
        );
        assert_eq!(
            user_option_display_format("#{@cdamux_agent}"),
            "#{@cdamux_agent}"
        );
    }
}
