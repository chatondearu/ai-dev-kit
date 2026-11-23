//! Parse and list tmux sessions (CLI subprocess v0).

use crate::session::SessionState;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

/// Dedicated full-screen dashboard session; hidden from Fleet / Sessions lists.
pub const HOME_SESSION_NAME: &str = "cdamux";

/// Dedicated Fleet navigator session (#337); hidden from Fleet / Sessions lists.
/// Never join-pane into agent sessions — operators switch-client here instead.
pub const NAV_SESSION_NAME: &str = "cdamux-nav";

/// True for sessions that must not appear in Fleet / list (home + navigator).
pub fn is_hidden_fleet_session(name: &str) -> bool {
    name == HOME_SESSION_NAME || name == NAV_SESSION_NAME
}

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
        .filter(|s| !is_hidden_fleet_session(&s.name))
        .collect())
}

/// Like [`list_tmux_sessions`], but `None` whenever tmux cannot be listed (no server,
/// another socket, missing binary): callers that delete must then treat all as live.
pub fn list_tmux_sessions_strict() -> Option<Vec<TmuxSession>> {
    let out = Command::new("tmux")
        .args(["list-sessions", "-F", LIST_SESSIONS_FORMAT])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| parse_list_sessions(&String::from_utf8_lossy(&out.stdout)))
}

/// Per-session snapshot from one `tmux list-panes -a` call (idle + hook user options).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionPaneInfo {
    /// Seconds since the active pane's window last had output; `None` when unknown (do not treat as 0 → Working).
    pub idle_seconds: Option<u32>,
    /// Raw `@cdamux_state` set by hooks, if any (transitional until hooks write `CDAMUX_HOME`).
    pub hook_state: Option<String>,
    /// Stable agent id (`@cdamux_id`); used to read `agents/<id>.json` status.
    pub cdamux_id: Option<String>,
    pub cdamux_agent: Option<String>,
    pub ccmux_agent: Option<String>,
    /// True when the active pane still looks like an agent CLI.
    /// A shell / `mise` / `direnv` in the tty foreground group clears this even when
    /// `#{pane_current_command}` is still `cursor-agent`.
    pub has_agent_process: bool,
    /// True when at least one pane reported a non-empty `pane_current_command`.
    pub saw_pane_command: bool,
    /// `#{pane_pid}` of the active pane.
    pub pane_pid: Option<u32>,
    /// Comm of the active pane's tty foreground group, when readable.
    pub proc_comm: Option<String>,
}

/// Session user options resolve through pane formats, so one `-a` call covers every session.
/// tmux has no pane idle format (`pane_idle_time` does not exist and expands empty):
/// idle is `now - window_activity` (epoch of the window's last pane output).
const LIST_PANES_ALL_FORMAT: &str =
    "#{session_name}\t#{pane_active}\t#{window_activity}\t#{@cdamux_state}\t#{@cdamux_id}\t#{@cdamux_agent}\t#{@ccmux_agent}\t#{pane_current_command}\t#{pane_pid}";

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Parse [`LIST_PANES_ALL_FORMAT`] lines into a map keyed by session name; `now` is epoch seconds.
/// Idle comes from the active pane, else the session's first pane.
pub fn parse_list_panes_all(stdout: &str, now: u64) -> HashMap<String, SessionPaneInfo> {
    let mut out: HashMap<String, SessionPaneInfo> = HashMap::new();
    // Built on first use, once per call, only where `/proc` is missing (macOS).
    let ps_table = std::cell::OnceCell::new();
    for line in stdout.lines() {
        let mut f = line.split('\t');
        let (Some(name), Some(active), Some(idle)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let is_new = !out.contains_key(name);
        let info = out.entry(name.to_string()).or_default();
        if is_new || active == "1" {
            info.idle_seconds = idle
                .trim()
                .parse::<u64>()
                .ok()
                .map(|t| u32::try_from(now.saturating_sub(t)).unwrap_or(u32::MAX));
        }
        info.hook_state = f.next().and_then(non_empty);
        info.cdamux_id = f.next().and_then(non_empty);
        info.cdamux_agent = f.next().and_then(non_empty);
        info.ccmux_agent = f.next().and_then(non_empty);
        let cmd = f.next().and_then(non_empty);
        let pid = f
            .next()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|n| *n > 0);
        if is_new || active == "1" {
            info.pane_pid = pid;
            // pane_pid is the shell cdamux started; its own comm stays `fish` while
            // the agent runs. The tty foreground group is the process that is actually
            // in front. tmux can keep reporting `cursor-agent` after that group is gone.
            info.proc_comm = pid.and_then(|p| foreground_comm(p, &ps_table));
        }
        if let Some(cmd) = cmd {
            info.saw_pane_command = true;
            let comm = (is_new || active == "1")
                .then(|| info.proc_comm.as_deref())
                .flatten();
            if info.proc_comm.as_deref().is_some_and(is_dead_pane_comm) {
                // Active (or first) pane's real comm wins over a stale command string.
                info.has_agent_process = false;
            } else if pane_command_is_agent(&cmd, comm) {
                info.has_agent_process = true;
            }
        }
    }
    out
}

fn read_proc_comm(pid: u32) -> Option<String> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let comm = raw.trim();
    (!comm.is_empty()).then(|| comm.to_string())
}

/// `tpgid` from `/proc/<pid>/stat` (field after tty_nr). `comm` may contain `)`.
fn tpgid_from_stat(stat: &str) -> Option<u32> {
    let rest = stat.rsplit_once(')')?.1;
    let tpgid: i64 = rest.split_whitespace().nth(5)?.parse().ok()?;
    u32::try_from(tpgid).ok().filter(|n| *n > 0)
}

/// Comm of the pane's foreground process group, not of `pane_pid` itself: `/proc` on
/// Linux, else one `ps -A` snapshot shared by every pane of the call (macOS, L1).
fn foreground_comm(pane_pid: u32, ps_table: &std::cell::OnceCell<HashMap<u32, (u32, String)>>) -> Option<String> {
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pane_pid}/stat")) {
        return read_proc_comm(tpgid_from_stat(&stat)?);
    }
    let table = ps_table.get_or_init(ps_snapshot);
    let (tpgid, _) = table.get(&pane_pid)?;
    table.get(tpgid).map(|(_, comm)| comm.clone())
}

/// `pid → (tpgid, comm basename)` from `ps -A -o pid=,tpgid=,comm=`; empty on failure.
fn ps_snapshot() -> HashMap<u32, (u32, String)> {
    Command::new("ps")
        .args(["-A", "-o", "pid=,tpgid=,comm="])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_ps_table(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// macOS prints the full executable path as `comm` (with spaces, or `-fish` for a
/// login shell): keep the basename so it matches the `/proc/<pid>/comm` form.
fn parse_ps_table(text: &str) -> HashMap<u32, (u32, String)> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.trim_start().splitn(2, char::is_whitespace);
            let pid = it.next()?.parse().ok()?;
            let rest = it.next()?.trim_start();
            let mut it = rest.splitn(2, char::is_whitespace);
            let tpgid = it.next()?.parse::<i64>().ok()?;
            let comm = it.next()?.trim();
            let base = comm.rsplit('/').next().unwrap_or(comm);
            (tpgid > 0 && !base.is_empty()).then(|| (pid, (tpgid as u32, base.to_string())))
        })
        .collect()
}

/// One tmux spawn for all sessions (empty map when tmux is down or has no server).
pub fn list_panes_all() -> HashMap<String, SessionPaneInfo> {
    let out = Command::new("tmux")
        .args(["list-panes", "-a", "-F", LIST_PANES_ALL_FORMAT])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            parse_list_panes_all(&String::from_utf8_lossy(&o.stdout), now)
        }
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
        .args([
            "display-message",
            "-p",
            "-t",
            &pane_target(session_name),
            &format,
        ])
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

/// Login shells show as `-fish`, `-bash`, …
pub fn is_shell_command(cmd: &str) -> bool {
    matches!(
        cmd.trim_start_matches('-'),
        "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" | "mksh" | "tcsh" | "csh" | "nu"
    )
}

/// True when a process comm is a shell, `mise`, or `direnv` (the CLI has exited).
pub(crate) fn is_dead_pane_comm(comm: &str) -> bool {
    let c = comm.trim().trim_start_matches('-');
    is_shell_command(c) || matches!(c, "mise" | "direnv")
}

/// `pane_current_command` still says `cursor-agent` after the process became a shell.
/// `proc_comm` is that lie's correction; `None` keeps the tmux string (no `/proc`).
pub(crate) fn pane_command_is_agent(pane_command: &str, proc_comm: Option<&str>) -> bool {
    if proc_comm.is_some_and(is_dead_pane_comm) {
        return false;
    }
    looks_like_agent_process(pane_command)
}

/// True when `#{pane_current_command}` is the agent CLI, not a leftover shell / mise.
pub fn looks_like_agent_process(cmd: &str) -> bool {
    let c = cmd.trim_start_matches('-').trim();
    if c.is_empty() || is_shell_command(c) {
        return false;
    }
    // mise untrusted / direnv hook: the agent never started.
    !matches!(c, "mise" | "direnv")
}

/// Pane heuristics when no reported agent status exists (see architecture.md).
pub fn classify_pane_heuristics(
    idle_seconds: Option<u32>,
    pane_tail: &str,
    idle_seconds_for_needs_input: u32,
) -> SessionState {
    // Unknown idle → Idle (fail closed), never Working-via-zero.
    let Some(idle_seconds) = idle_seconds else {
        return SessionState::Idle;
    };
    let threshold = idle_seconds_for_needs_input.max(1);
    if pane_tail_looks_like_input(pane_tail) && idle_seconds >= threshold {
        return SessionState::NeedsInput;
    }
    if idle_seconds <= 1 {
        return SessionState::Working;
    }
    if idle_seconds < threshold {
        return SessionState::Working;
    }
    SessionState::Idle
}

pub(crate) fn pane_tail_looks_like_input(tail: &str) -> bool {
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
        .args([
            "capture-pane",
            "-p",
            "-t",
            &pane_target(session_name),
            "-S",
            "-30",
        ])
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
    args.extend([
        "-t".into(),
        pane_target(session_name),
        "-S".into(),
        format!("-{}", max_lines.max(1)),
    ]);
    args
}

/// Capture active pane text for a session (negative start = lines from bottom).
pub fn capture_pane_text(
    session_name: &str,
    max_lines: i32,
    escapes: bool,
) -> Result<String, TmuxError> {
    let args = capture_pane_args(session_name, max_lines, escapes);
    let capture = Command::new("tmux").args(&args).output().map_err(|e| {
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

/// Test shorthand: [`input_argvs`] on a session's active pane, no named keys.
#[cfg(test)]
fn send_line_argvs(session_name: &str, line: &str, enter: bool) -> Vec<Vec<String>> {
    input_argvs(&pane_target(session_name), line, &[], enter, "cdamux-send-test")
}

/// Control characters text may not carry into a pane: they act as keys (`\r` submits,
/// `\x03` interrupts, ESC starts sequences) and would bypass the `--key` allow-list.
/// `\n` and `\t` stay allowed for multi-line prompts.
pub(crate) fn is_forbidden_control(c: char) -> bool {
    c.is_control() && c != '\n' && c != '\t'
}

/// tmux argv per invocation needed to type `line`, press named `keys` (one call each)
/// and optionally submit it, on any tmux `target`.
///
/// Text always goes through a uniquely named paste buffer (`load-buffer -` reads stdin),
/// pasted with `-p` (bracketed paste) and `-d` (buffer deleted once pasted). Bracketed
/// paste only applies when the receiving app enabled it; without it, each `\n` still
/// submits a line. Keys and `Enter` are separate `send-keys` calls, never `-l`.
/// Nothing clears the input line first: a human draft is never wiped.
fn input_argvs(
    target: &str,
    line: &str,
    keys: &[&str],
    enter: bool,
    buffer: &str,
) -> Vec<Vec<String>> {
    let target = target.to_string();
    let argv = |args: &[&str]| -> Vec<String> { args.iter().map(|s| s.to_string()).collect() };
    let mut out = Vec::new();
    if !line.is_empty() {
        out.push(argv(&["load-buffer", "-b", buffer, "-"]));
        out.push(argv(&[
            "paste-buffer",
            "-p",
            "-d",
            "-b",
            buffer,
            "-t",
            &target,
        ]));
    }
    for key in keys {
        out.push(argv(&["send-keys", "-t", &target, key]));
    }
    if enter {
        out.push(argv(&["send-keys", "-t", &target, "Enter"]));
    }
    out
}

/// Paste buffer name unique per call: concurrent sends never paste each other's text.
fn paste_buffer_name() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("cdamux-send-{}-{n}", std::process::id())
}

/// Pause between pane writes. Cursor CLI reads bytes that land in one burst as a paste:
/// text + Enter becomes a newline (no submit), C-u + text drops the text. ≥100 ms also
/// lets Cursor clear its composer after submit.
const SEND_SETTLE: std::time::Duration = std::time::Duration::from_millis(300);

/// Per argv: sleep [`SEND_SETTLE`] first? True for a pane write after an earlier one.
fn settle_before(argvs: &[Vec<String>]) -> Vec<bool> {
    let mut wrote = false;
    argvs
        .iter()
        .map(|a| {
            let pane_write = a[0] != "load-buffer";
            let settle = pane_write && wrote;
            wrote |= pane_write;
            settle
        })
        .collect()
}

/// Send a literal line to the session's active pane; optional Enter keypress.
pub fn send_line_to_session(session_name: &str, line: &str, enter: bool) -> Result<(), TmuxError> {
    run_input(&pane_target(session_name), line, &[], enter)
}

/// Type `line`, then press `keys` (tmux key names, never `-l`), then optional Enter on `target`.
/// Callers must pass allow-listed key names only (`send::SEND_KEYS`).
pub(crate) fn send_input_to_target(
    target: &str,
    line: &str,
    keys: &[&str],
    enter: bool,
) -> Result<(), TmuxError> {
    run_input(target, line, keys, enter)
}

fn run_input(target: &str, line: &str, keys: &[&str], enter: bool) -> Result<(), TmuxError> {
    let line: String = line.chars().filter(|c| !is_forbidden_control(*c)).collect();
    let buffer = paste_buffer_name();
    let argvs = input_argvs(target, &line, keys, enter, &buffer);
    let result = run_argvs(&argvs, &line);
    if result.is_err() && !line.is_empty() {
        // `paste-buffer -d` only deletes on success: never leave the text (maybe a secret) behind.
        let _ = Command::new("tmux")
            .args(["delete-buffer", "-b", &buffer])
            .stderr(Stdio::null())
            .status();
    }
    result
}

fn run_argvs(argvs: &[Vec<String>], line: &str) -> Result<(), TmuxError> {
    let map_err = |e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TmuxError::NotFound
        } else {
            TmuxError::ListFailed
        }
    };
    for (argv, settle) in argvs.iter().zip(settle_before(argvs)) {
        if settle {
            std::thread::sleep(SEND_SETTLE);
        }
        let mut cmd = Command::new("tmux");
        cmd.args(argv);
        let ok = if argv[0] == "load-buffer" {
            let mut child = cmd.stdin(Stdio::piped()).spawn().map_err(map_err)?;
            // Dropping stdin after the write closes the pipe so load-buffer sees EOF.
            let wrote = child
                .stdin
                .take()
                .map(|mut i| i.write_all(line.as_bytes()).is_ok());
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

    /// argv pair that pastes the test buffer onto `target`.
    fn paste(target: &str) -> Vec<Vec<&str>> {
        vec![
            vec!["load-buffer", "-b", "cdamux-send-test", "-"],
            vec![
                "paste-buffer",
                "-p",
                "-d",
                "-b",
                "cdamux-send-test",
                "-t",
                target,
            ],
        ]
    }

    #[test]
    fn send_line_enter_is_separate_key_invocation() {
        let mut want = paste("=c-bar:");
        want.push(vec!["send-keys", "-t", "=c-bar:", "Enter"]);
        // Exactly paste + Enter: no `C-u` first, a human draft is never wiped.
        assert_eq!(send_line_argvs("c-bar", "echo hi", true), want);
        assert_eq!(send_line_argvs("c-bar", "y", false).len(), 2);
        assert_eq!(send_line_argvs("c-bar", "", true).len(), 1); // Enter only
    }

    #[test]
    fn text_never_goes_through_send_keys() {
        for line in ["y", "a\nb", "-la", "C-c"] {
            let argvs = send_line_argvs("c-bar", line, true);
            assert!(
                argvs.iter().all(|a| !a.contains(&line.to_string())),
                "{line}: {argvs:?}"
            );
            assert!(
                argvs.iter().all(|a| !a.contains(&"-l".to_string())),
                "{line}: {argvs:?}"
            );
        }
    }

    #[test]
    fn control_chars_are_forbidden_except_newline_and_tab() {
        for c in ['\r', '\x03', '\x1b', '\x04', '\x7f', '\u{9b}'] {
            assert!(is_forbidden_control(c), "{c:?}");
        }
        for c in ['\n', '\t', 'a', ' ', '📨'] {
            assert!(!is_forbidden_control(c), "{c:?}");
        }
    }

    #[test]
    fn paste_buffer_names_are_unique() {
        assert_ne!(paste_buffer_name(), paste_buffer_name());
        assert!(paste_buffer_name().starts_with("cdamux-send-"));
    }

    #[test]
    fn input_keys_go_between_text_and_enter_without_literal_flag() {
        let mut want = paste("%3");
        want.push(vec!["send-keys", "-t", "%3", "C-c"]);
        want.push(vec!["send-keys", "-t", "%3", "Up"]);
        want.push(vec!["send-keys", "-t", "%3", "Enter"]);
        assert_eq!(
            input_argvs("%3", "y", &["C-c", "Up"], true, "cdamux-send-test"),
            want
        );
    }

    #[test]
    fn send_settles_between_pane_writes() {
        let s = |line, enter| settle_before(&send_line_argvs("c-bar", line, enter));
        // load-buffer is not a pane write: no pause before paste-buffer
        assert_eq!(s("hi", true), vec![false, false, true]); // pause before Enter
        assert_eq!(s("hi", false), vec![false, false]);
        assert_eq!(s("", true), vec![false]);
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
        assert_eq!(NAV_SESSION_NAME, "cdamux-nav");
        let raw = "cdamux\t/\ncdamux-nav\t/\nc-foo\t/home/foo\n";
        let visible: Vec<_> = parse_list_sessions(raw)
            .into_iter()
            .filter(|s| !is_hidden_fleet_session(&s.name))
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
            SessionState::Working
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
            SessionState::Working
        );
    }

    #[test]
    fn parse_ps_table_keeps_basename_and_drops_no_tty() {
        let t = parse_ps_table(
            "  1     0 /sbin/launchd\n 4242  4300 -fish\n 4300  4300 /opt/My Apps/bin/claude\nbad line\n",
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t[&4242], (4300, "-fish".to_string()));
        assert_eq!(t[&4300], (4300, "claude".to_string()));
        assert!(is_dead_pane_comm(&t[&4242].1));
    }

    #[test]
    fn parse_list_panes_all_keys_by_session() {
        // Third field is window_activity (epoch); idle = now - it. Active pane wins;
        // blank activity → None; blank options → None.
        // Fields: name, active, window_activity, @cdamux_state, @cdamux_id, @cdamux_agent, @ccmux_agent
        let raw = "a\t0\t901\t\t\t\t\nb\t1\t958\twaiting\ta-1\tclaude\tcursor\nb\t0\t993\twaiting\ta-1\tclaude\tcursor\na\t1\t\t\t\t\tpi\n";
        let map = parse_list_panes_all(raw, 1000);
        assert_eq!(map.len(), 2);
        let a = &map["a"];
        assert_eq!(a.idle_seconds, None);
        assert_eq!(a.hook_state, None);
        assert_eq!(a.cdamux_id, None);
        assert_eq!(a.cdamux_agent, None);
        assert_eq!(a.ccmux_agent.as_deref(), Some("pi"));
        let b = &map["b"];
        assert_eq!(b.idle_seconds, Some(42));
        assert_eq!(b.hook_state.as_deref(), Some("waiting"));
        assert_eq!(b.cdamux_id.as_deref(), Some("a-1"));
        assert_eq!(b.cdamux_agent.as_deref(), Some("claude"));
        // No active pane → first pane's idle.
        let first = parse_list_panes_all("c\t0\t995\t\t\t\t\nc\t0\t991\t\t\t\t\n", 1000);
        assert_eq!(first["c"].idle_seconds, Some(5));
        // Activity stamped after `now` (clock race) saturates to 0, not a panic.
        assert_eq!(
            parse_list_panes_all("d\t1\t1002\t\t\t\t\n", 1000)["d"].idle_seconds,
            Some(0)
        );
        assert!(parse_list_panes_all("", 1000).is_empty());
        let dead = parse_list_panes_all("e\t1\t990\t\ta-e\tcursor\t\tfish\n", 1000);
        assert!(dead["e"].saw_pane_command && !dead["e"].has_agent_process);
        let live = parse_list_panes_all(
            "f\t1\t990\t\ta-f\tcursor\t\tfish\nf\t0\t990\t\ta-f\tcursor\t\tcursor-agent\n",
            1000,
        );
        assert!(live["f"].has_agent_process);
    }

    #[test]
    fn fish_comm_overrides_cursor_agent_command() {
        assert!(!pane_command_is_agent("cursor-agent", Some("fish")));
        assert!(!pane_command_is_agent("cursor-agent", Some("mise")));
        assert!(!pane_command_is_agent("cursor-agent", Some("direnv")));
        assert!(!pane_command_is_agent("cursor-agent", Some("-zsh")));
        assert!(pane_command_is_agent("cursor-agent", Some("cursor-agent")));
        assert!(pane_command_is_agent("cursor-agent", None));
        assert!(pane_command_is_agent("cursor-agent", Some("node")));
    }

    #[test]
    fn stat_tpgid_is_the_foreground_group_not_the_shell_pid() {
        // pid (comm) state ppid pgrp session tty_nr tpgid …
        assert_eq!(
            tpgid_from_stat("2811669 (fish) S 1 2811669 2811669 34860 2837251 0"),
            Some(2837251)
        );
        assert_eq!(tpgid_from_stat("10 (a) b) S 1 2 3 4 -1"), None);
    }

    #[test]
    fn looks_like_agent_process_rejects_shell_and_mise() {
        assert!(looks_like_agent_process("cursor-agent"));
        assert!(looks_like_agent_process("claude"));
        assert!(!looks_like_agent_process("fish"));
        assert!(!looks_like_agent_process("-zsh"));
        assert!(!looks_like_agent_process("mise"));
        assert!(!looks_like_agent_process(""));
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
