//! Inbox messages under `$CDAMUX_HOME/inbox/` (ADR-011 §4).
//! JSONL ≤ 2 KB per line; ack cursor is `<agent-id>.cursor` (byte offset).

use crate::agent_status::{may_overwrite, parse_reported_status};
use crate::paths::cdamux_state_dir;
use crate::send::send_to_session;
use crate::session::SessionState;
use crate::session_registry;
use crate::state::{
    random_chars, state_append, state_get, state_list, try_entity_lock, valid_state_id,
    with_entity_lock, write_atomic, StateKind, BASE36,
};
use crate::tmux::{list_panes_all, pane_tail, pane_tail_looks_like_input};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Max serialised JSON line size (ADR-011 §4).
pub const MAX_MSG_BYTES: usize = 2048;

/// Quiet pane window before `say` pastes a human prompt, and of the worker cap (seconds).
pub const DEFAULT_NUDGE_QUIET_SECS: u32 = 10;

/// Quiet pane window before the wake line is typed into an agent at rest (seconds).
/// `CDAMUX_WAKE_QUIET_SECS` overrides it (tuning, tmux e2e tests); the idle time itself is
/// always read from tmux.
pub const DEFAULT_WAKE_QUIET_SECS: u32 = 60;

/// A background waker gives up after this long; the agent's next prompt or turn end
/// still hands the inbox over through its hooks.
const WAKER_MAX: Duration = Duration::from_secs(6 * 3600);

/// Effective wake quiet window: `CDAMUX_WAKE_QUIET_SECS` when set to a positive number.
pub fn wake_quiet_secs() -> u32 {
    std::env::var("CDAMUX_WAKE_QUIET_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_WAKE_QUIET_SECS)
}

const KINDS: &[&str] = &["report", "ask", "done", "error", "relay", "answer"];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MsgError {
    #[error("{0}")]
    Invalid(String),
    #[error("message exceeds {MAX_MSG_BYTES} bytes")]
    TooLarge,
    #[error("agent not found: {0}")]
    NotFound(String),
    #[error("ambiguous target '{0}' (multiple agents); use an agent id")]
    Ambiguous(String),
    #[error("io: {0}")]
    Io(String),
}

impl From<io::Error> for MsgError {
    fn from(e: io::Error) -> Self {
        MsgError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub ts: u64,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

/// What `msg send` did beyond the inbox append (Q21: a message body is never typed into
/// a pane; only the fixed wake line is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The agent was at rest on a quiet pane: the wake line was typed.
    Woken,
    /// At rest but the pane is not quiet yet: a background waker retries.
    Scheduled,
    /// Mid-turn: the agent's `Stop` hook hands the messages over at turn end.
    TurnEnd,
    /// No live pane, or no safe moment known: the message waits in the inbox.
    Inbox,
}

impl Delivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Delivery::Woken => "woken",
            Delivery::Scheduled => "scheduled",
            Delivery::TurnEnd => "turn-end",
            Delivery::Inbox => "inbox",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendOutcome {
    pub message: Message,
    pub delivery: Delivery,
}

impl SendOutcome {
    pub fn delivery_word(&self) -> &'static str {
        self.delivery.as_str()
    }
}

fn inbox_dir() -> io::Result<PathBuf> {
    cdamux_state_dir("inbox").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CDAMUX_HOME / XDG_STATE_HOME / HOME unset",
        )
    })
}

pub(crate) fn inbox_path(agent_id: &str) -> io::Result<PathBuf> {
    if !valid_state_id(agent_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid agent id '{agent_id}'"),
        ));
    }
    Ok(inbox_dir()?.join(format!("{agent_id}.jsonl")))
}

fn cursor_path(agent_id: &str) -> io::Result<PathBuf> {
    if !valid_state_id(agent_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid agent id '{agent_id}'"),
        ));
    }
    Ok(inbox_dir()?.join(format!("{agent_id}.cursor")))
}

/// Mint `m-ttttttrrrrr`: 6 base36 chars of seconds since 2024-01-01 (sortable, good until
/// 2093) and 5 random ones, so two ids can collide only within the same second (1 in
/// 60 M per pair). `None` without `/dev/urandom`.
pub fn generate_msg_id() -> Option<String> {
    Some(format!("m-{}{}", base36_secs(now_ts()), random_chars::<5>()?))
}

/// Seconds since 2024-01-01 UTC as 6 base36 chars.
fn base36_secs(ts: u64) -> String {
    let mut n = ts.saturating_sub(1_704_067_200);
    let mut out = [b'0'; 6];
    for slot in out.iter_mut().rev() {
        *slot = BASE36[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn valid_kind(kind: &str) -> bool {
    KINDS.contains(&kind)
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Resolve `--to` as an agent id or a tmux / registry session name.
///
/// Logical `name` is unique only under a parent orchestrator, so a bare name that
/// matches several agents is an error unless `$CDAMUX_AGENT_ID` narrows it to one child.
pub fn resolve_agent_id(to: &str) -> Result<String, MsgError> {
    let to = to.trim();
    if to.is_empty() || !valid_state_id(to) {
        return Err(MsgError::Invalid(format!("invalid target '{to}'")));
    }
    if state_get(StateKind::Agents, to)
        .map_err(|e| MsgError::Io(e.to_string()))?
        .is_some()
    {
        return Ok(to.to_string());
    }
    if let Some(rec) = session_registry::load(to) {
        if let Some(id) = rec.cdamux_id.filter(|s| !s.is_empty()) {
            return Ok(id);
        }
    }
    // Scan agents for matching host (session name) or logical worker name.
    let matches: Vec<(String, Option<String>)> = state_list(StateKind::Agents)
        .into_iter()
        .filter(|(_, _, v)| {
            let host = v.get("host").and_then(|h| h.as_str());
            let name = v.get("name").and_then(|n| n.as_str());
            host == Some(to) || name == Some(to)
        })
        .map(|(id, _, v)| {
            let parent = v
                .get("parent")
                .and_then(|p| p.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            (id, parent)
        })
        .collect();
    match matches.as_slice() {
        [] => Err(MsgError::NotFound(to.to_string())),
        [(id, _)] => Ok(id.clone()),
        many => {
            let caller = std::env::var("CDAMUX_AGENT_ID")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if let Some(caller) = caller.as_deref() {
                let kids: Vec<&str> = many
                    .iter()
                    .filter(|(_, p)| p.as_deref() == Some(caller))
                    .map(|(id, _)| id.as_str())
                    .collect();
                if kids.len() == 1 {
                    return Ok(kids[0].to_string());
                }
            }
            Err(MsgError::Ambiguous(to.to_string()))
        }
    }
}

pub(crate) fn agent_role_and_host(id: &str) -> Result<(String, Option<String>), MsgError> {
    let v = state_get(StateKind::Agents, id)
        .map_err(|e| MsgError::Io(e.to_string()))?
        .ok_or_else(|| MsgError::NotFound(id.to_string()))?;
    let role = v
        .get("role")
        .and_then(|r| r.as_str())
        .unwrap_or("agent")
        .to_string();
    let host = v
        .get("host")
        .and_then(|h| h.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok((role, host))
}

/// `say` pastes a human prompt into an orchestrator only after a finished turn (`idle`, or
/// sticky `done` / `error`; no status = no hooks, so the quiet pane alone decides) and the
/// quiet window: the paste ends with Enter, so on a busy, a just-active or a
/// permission-prompt pane it would merge with (and submit) what the user is typing, or
/// answer the prompt.
pub fn should_paste(
    status: Option<SessionState>,
    idle_seconds: Option<u32>,
    quiet_secs: u32,
) -> bool {
    matches!(
        status,
        None | Some(SessionState::Idle | SessionState::Done | SessionState::Error)
    ) && idle_seconds.is_some_and(|s| s >= quiet_secs.max(1))
}

/// The fixed wake line. `injected`: the agent's `UserPromptSubmit` hook adds the bodies to
/// the prompt (and acks them); otherwise the line tells the agent to read its inbox.
pub fn wake_text(unread: usize, injected: bool) -> String {
    let n = if unread == 1 { "1 message".to_string() } else { format!("{unread} messages") };
    if injected {
        format!("📨 {n}")
    } else {
        format!("📨 {n} — cdamux msg read --ack")
    }
}

fn read_cursor(path: &Path) -> io::Result<u64> {
    match fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
        Ok(s) => Ok(s.trim().parse::<u64>().unwrap_or(0)),
    }
}

fn write_cursor(path: &Path, offset: u64) -> io::Result<()> {
    write_atomic(path, format!("{offset}\n").as_bytes())
}

/// Append one message line (O_APPEND). Rejects lines over [`MAX_MSG_BYTES`].
/// Returns the byte offset just past the appended line.
pub fn append_message(msg: &Message) -> Result<u64, MsgError> {
    let line = serde_json::to_string(msg).map_err(|e| MsgError::Io(e.to_string()))?;
    if line.len() > MAX_MSG_BYTES {
        return Err(MsgError::TooLarge);
    }
    let path = inbox_path(&msg.to)?;
    if let Some(dir) = path.parent() {
        crate::paths::create_private_dir(dir)?;
    }
    let mut f = crate::paths::private_file_options().append(true).open(&path)?;
    // One write(2) so concurrent O_APPEND senders cannot interleave line and newline.
    f.write_all(format!("{line}\n").as_bytes())?;
    // O_APPEND leaves the fd offset at the end of this write, even with concurrent writers.
    Ok(f.stream_position()?)
}

/// Read unread messages after the ack cursor. When `ack`, advance the cursor past the
/// last complete line read; a trailing partial line (no newline yet) stays unread.
pub fn read_messages(agent_id: &str, ack: bool) -> Result<Vec<Message>, MsgError> {
    read_messages_max(agent_id, ack, None)
}

/// [`read_messages`] stopping before the line that would pass `max_bytes` of inbox JSON
/// (always at least one line); `ack` acks only the lines returned. Hooks use it so a hook
/// output cap never drops a line that was already acked.
pub fn read_messages_max(
    agent_id: &str,
    ack: bool,
    max_bytes: Option<usize>,
) -> Result<Vec<Message>, MsgError> {
    if !valid_state_id(agent_id) {
        return Err(MsgError::Invalid(format!("invalid agent id '{agent_id}'")));
    }
    let path = inbox_path(agent_id)?;
    let cur_path = cursor_path(agent_id)?;
    let offset = read_cursor(&cur_path)?;
    let mut f = match OpenOptions::new().read(true).open(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(f) => f,
    };
    let meta = f.metadata()?;
    let len = meta.len();
    if offset > len {
        // Truncated inbox: reset cursor.
        if ack {
            write_cursor(&cur_path, len)?;
        }
        return Ok(Vec::new());
    }
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let mut consumed = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    if let Some(max) = max_bytes {
        let mut end = 0;
        for (i, _) in buf[..consumed].iter().enumerate().filter(|(_, &b)| b == b'\n') {
            if end > 0 && i + 1 > max {
                break;
            }
            end = i + 1;
        }
        consumed = end;
    }
    let mut out = Vec::new();
    for line in String::from_utf8_lossy(&buf[..consumed]).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Message>(line) {
            Ok(m) => out.push(m),
            Err(_) => continue,
        }
    }
    if ack {
        write_cursor(&cur_path, offset + consumed as u64)?;
    }
    Ok(out)
}

/// Count unread messages (no ack).
pub fn unread_count(agent_id: &str) -> Result<usize, MsgError> {
    Ok(read_messages(agent_id, false)?.len())
}

/// Every inbox line for `agent_id` (ignores the ack cursor; does not advance it).
pub fn read_inbox_all(agent_id: &str) -> Result<Vec<Message>, MsgError> {
    if !valid_state_id(agent_id) {
        return Err(MsgError::Invalid(format!("invalid agent id '{agent_id}'")));
    }
    let path = inbox_path(agent_id)?;
    let text = match fs::read_to_string(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(t) => t,
    };
    Ok(text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty())
                .then(|| serde_json::from_str::<Message>(line).ok())
                .flatten()
        })
        .collect())
}

/// Parent agent id from `agents/<id>.json`, when set.
pub fn agent_parent_id(agent_id: &str) -> Option<String> {
    state_get(StateKind::Agents, agent_id)
        .ok()
        .flatten()
        .and_then(|v| {
            v.get("parent")
                .and_then(|p| p.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// True when the worker's inbox answers `ask`: an `answer` or `relay` whose `ref` is a
/// message id answers that ask only (`msg answer`); any other one (no `ref`, or a `--ref`
/// path) from the asked agent answers every ask the worker sent it up to then (TUI /
/// tailnet reply, or an orchestrator replying with `relay`; it also lifts `blocked`).
pub fn ask_answered(ask: &Message, worker_msgs: &[Message]) -> bool {
    worker_msgs.iter().any(|ans| {
        (ans.kind == "answer" || ans.kind == "relay")
            && match ans.r#ref.as_deref().filter(|r| r.starts_with("m-")) {
                Some(r) => r == ask.id,
                None => ans.from == ask.to && ans.ts >= ask.ts,
            }
    })
}

/// True when `worker` sent an `ask` to `parent` with no later `answer` back.
pub fn has_pending_ask(worker_id: &str, parent_id: &str) -> bool {
    let Ok(parent_msgs) = read_inbox_all(parent_id) else {
        return false;
    };
    let Ok(worker_msgs) = read_inbox_all(worker_id) else {
        return false;
    };
    parent_msgs
        .iter()
        .any(|ask| ask.kind == "ask" && ask.from == worker_id && !ask_answered(ask, &worker_msgs))
}

/// The inbox line with this message id, wherever it was delivered.
pub fn find_message(id: &str) -> Result<Option<Message>, MsgError> {
    if !valid_state_id(id) {
        return Err(MsgError::Invalid(format!("invalid message id '{id}'")));
    }
    let entries = match fs::read_dir(inbox_dir()?) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        r => r?,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            let owner = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            if let Some(m) = read_inbox_all(owner)?.into_iter().find(|m| m.id == id) {
                return Ok(Some(m));
            }
        }
    }
    Ok(None)
}

/// Inbox badges for one agent: unread in its own inbox, and (for workers) a
/// pending ask in the parent's inbox (any cursor position; not only unread).
pub fn inbox_badges(agent_id: &str) -> (usize, bool) {
    let unread = unread_count(agent_id).unwrap_or(0);
    let pending_ask = agent_parent_id(agent_id)
        .map(|p| has_pending_ask(agent_id, &p))
        .unwrap_or(false);
    (unread, pending_ask)
}

/// Orchestrator answers a worker ask (`kind=answer`); `from` is the parent id.
pub fn reply_to_ask(worker_id: &str, text: &str) -> Result<SendOutcome, MsgError> {
    let parent = agent_parent_id(worker_id).ok_or_else(|| {
        MsgError::Invalid(format!(
            "agent '{worker_id}' has no parent; cannot answer ask"
        ))
    })?;
    msg_send(&parent, worker_id, "answer", text, None, None)
}

fn agent_status(id: &str) -> Result<Option<SessionState>, MsgError> {
    Ok(state_get(StateKind::Agents, id)
        .map_err(|e| MsgError::Io(e.to_string()))?
        .and_then(|v| v.get("status").and_then(|s| s.as_str()).and_then(parse_reported_status)))
}

fn set_status(id: &str, status: &str) -> Result<(), MsgError> {
    state_append(StateKind::Agents, id, &json!({ "status": status }))
        .map_err(|e| MsgError::Io(e.to_string()))
}

/// Status side effects of a message. `ask` sets the sender `blocked` (shown as `?`); a
/// parent's `answer` or `relay` lifts the recipient's `blocked` / `done` / `error` to
/// `idle` (follow-up on a finished live pane, ADR-018 amendment); the blocked agent's
/// own `done` replaces `blocked` (it finished anyway). `error` keeps a sticky status:
/// orch.sh reports its `blocked` workers as kind=error.
fn apply_kind_status(kind: &str, from: &str, to: &str) -> Result<(), MsgError> {
    match kind {
        // Sticky: an ask never turns a finished `done`/`error` back into `blocked`.
        "ask" => crate::state::state_append_sticky(StateKind::Agents, from, &json!({ "status": "blocked" }))
            .map_err(|e| MsgError::Io(e.to_string()))?,
        "done" | "error" => {
            let known = state_get(StateKind::Agents, from)
                .map_err(|e| MsgError::Io(e.to_string()))?
                .is_some();
            let cur = agent_status(from)?;
            let next = if kind == "done" { SessionState::Done } else { SessionState::Error };
            let own_done = kind == "done" && cur == Some(SessionState::Blocked);
            if known && (own_done || may_overwrite(cur, next)) {
                set_status(from, kind)?;
            }
        }
        "answer" | "relay"
            if matches!(
                agent_status(to)?,
                Some(SessionState::Blocked | SessionState::Done | SessionState::Error)
            ) =>
        {
            set_status(to, "idle")?;
        }
        _ => {}
    }
    Ok(())
}

/// Background wakers are spawned only from the `cdamux` binary (it calls
/// [`enable_background_waker`]); library callers and tests get `scheduled` without one.
static BACKGROUND_WAKER: AtomicBool = AtomicBool::new(false);

/// Let [`msg_send`] start `cdamux msg wake <id>` (the current executable) when an agent at
/// rest is not quiet yet.
pub fn enable_background_waker() {
    BACKGROUND_WAKER.store(true, Ordering::Relaxed);
}

fn spawn_waker(agent_id: &str) {
    use std::os::unix::process::CommandExt;
    if !BACKGROUND_WAKER.load(Ordering::Relaxed) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    // Own process group, no stdio: it outlives the caller (a hook, a tool call).
    let child = std::process::Command::new(exe)
        .args(["msg", "wake", agent_id])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
    // Reap it in a long-lived caller (TUI); a short CLI exits first, init reaps it then.
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// Validate, append, apply status side effects, then wake the recipient when it is at rest.
/// The body is never typed into a pane (Q21): mid-turn the `Stop` hook hands it over, an
/// agent at rest gets only the wake line ([`wake_once`]), retried in the background
/// until its pane has been quiet for [`wake_quiet_secs`].
///
/// `idle_override` injects pane idle seconds in unit tests (`None` → read from tmux).
pub fn msg_send(
    from: &str,
    to_raw: &str,
    kind: &str,
    text: &str,
    r#ref: Option<&str>,
    idle_override: Option<Option<u32>>,
) -> Result<SendOutcome, MsgError> {
    if !valid_kind(kind) {
        return Err(MsgError::Invalid(format!(
            "kind must be one of {}",
            KINDS.join("|")
        )));
    }
    check_text(text)?;
    if !valid_state_id(from) {
        return Err(MsgError::Invalid(format!("invalid from id '{from}'")));
    }
    let to = resolve_agent_id(to_raw)?;
    let id = generate_msg_id().ok_or_else(|| MsgError::Io("urandom unavailable".into()))?;
    let msg = Message {
        id,
        ts: now_ts(),
        from: from.to_string(),
        to: to.clone(),
        kind: kind.to_string(),
        text: text.to_string(),
        r#ref: r#ref.map(str::to_string),
    };
    // Size check before side-effects.
    let line = serde_json::to_string(&msg).map_err(|e| MsgError::Io(e.to_string()))?;
    if line.len() > MAX_MSG_BYTES {
        return Err(MsgError::TooLarge);
    }

    append_message(&msg)?;
    apply_kind_status(kind, from, &to)?;
    let (role, _) = agent_role_and_host(&to)?;
    if role == "orchestrator" {
        crate::events::emit_msg(&msg);
    }

    // Inbox append is the source of truth; the wake line is best-effort.
    let delivery = match wake_once(&to, wake_quiet_secs(), idle_override) {
        Ok(WakePlan::Now) => Delivery::Woken,
        Ok(WakePlan::After(_)) => {
            spawn_waker(&to);
            Delivery::Scheduled
        }
        Ok(WakePlan::TurnEnd) => {
            // Backstop: a line appended while the Stop hook already ran waits for a wake.
            spawn_waker(&to);
            Delivery::TurnEnd
        }
        Ok(WakePlan::Never) | Err(_) => Delivery::Inbox,
    };
    crate::hooklog::hook_log(
        "msg",
        &to,
        delivery.as_str(),
        serde_json::json!({ "id": msg.id, "kind": msg.kind, "from": msg.from }),
    );
    Ok(SendOutcome { message: msg, delivery })
}

/// Non-empty, at most [`MAX_MSG_BYTES`], and free of control characters other than `\n` /
/// `\t` (an ESC would drive a terminal that prints the inbox).
pub(crate) fn check_text(s: &str) -> Result<(), MsgError> {
    if s.len() > MAX_MSG_BYTES {
        return Err(MsgError::TooLarge);
    }
    if s.trim().is_empty() || s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(MsgError::Invalid(
            "text must be non-empty, without control characters other than newline and tab"
                .into(),
        ));
    }
    Ok(())
}

/// What a wake attempt does for an agent with unread messages and a live pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakePlan {
    /// Type the wake line now.
    Now,
    /// Retry after that many seconds (not quiet yet, or busy without turn hooks).
    After(u32),
    /// Mid-turn: the `Stop` hook delivers (a waker still polls as a backstop).
    TurnEnd,
    /// Nothing to wake, or no safe moment is knowable.
    Never,
}

/// Wake decision (Q21, ADR-018 amendment). `turn` is the hook-written field: `running`
/// from prompt submit or a permission request until `Stop`, then `ended`; `exited` after
/// the CLI's session end (the pane may be a shell now). When `turn` is `Some`, every
/// client uses the same branch: `running` is never typed into (permission dialogs too;
/// `Stop` / a later ended turn delivers), `ended` is at rest except `working` /
/// `needs_input`. A Claude agent with a status but no `turn` yet has not submitted its
/// first prompt (startup dialogs): wait. Other clients with no `turn`: `idle` / `done` /
/// `error` / unset are at rest on a live pane; `blocked` stays inbox until `relay` /
/// `answer`. `working` / `needs_input` / `pane_input` still hold the wake.
pub fn wake_plan(
    claude: bool,
    turn: Option<&str>,
    status: Option<SessionState>,
    pane_input: bool,
    idle: Option<u32>,
    quiet: u32,
) -> WakePlan {
    let quiet = quiet.max(1);
    let at_rest = match (claude, turn) {
        (_, Some("exited")) => return WakePlan::Never,
        (_, Some("running")) => return WakePlan::TurnEnd,
        (_, Some(_)) => !matches!(status, Some(SessionState::NeedsInput | SessionState::Working)),
        (true, None) if status.is_some() => false,
        _ => match status {
            None | Some(SessionState::Idle | SessionState::Done | SessionState::Error) => true,
            Some(SessionState::Working | SessionState::NeedsInput) => false,
            Some(_) => return WakePlan::Never,
        },
    };
    match idle {
        _ if !at_rest || pane_input => WakePlan::After(quiet),
        Some(s) if s >= quiet => WakePlan::Now,
        Some(s) => WakePlan::After(quiet - s),
        None => WakePlan::After(quiet),
    }
}

fn inbox_len(agent_id: &str) -> Result<u64, MsgError> {
    match fs::metadata(inbox_path(agent_id)?) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        r => Ok(r?.len()),
    }
}

/// `inbox/<id>.<ext>`: lock entities next to the inbox (validated id).
fn inbox_sibling(agent_id: &str, ext: &str) -> Result<PathBuf, MsgError> {
    Ok(inbox_path(agent_id)?.with_extension(ext))
}

/// One wake attempt under the agent's wake lock: reads unread count, record and pane, then
/// types [`wake_text`] (never a body, never `C-u`) when [`wake_plan`] says `Now`. A typed
/// line refreshes the pane's activity, so a concurrent attempt waits for the next quiet
/// window. Returns the plan acted on; a failed type is an error.
pub fn wake_once(
    agent_id: &str,
    quiet: u32,
    idle_override: Option<Option<u32>>,
) -> Result<WakePlan, MsgError> {
    let lock = inbox_sibling(agent_id, "wake")?;
    with_entity_lock(&lock, || {
        let unread = unread_count(agent_id).map_err(|e| io::Error::other(e.to_string()))?;
        let rec = state_get(StateKind::Agents, agent_id)?.unwrap_or_default();
        let field = |k: &str| rec.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty());
        // A gone pane ends the waker too: the inbox waits for a restore.
        let Some(host) = field("host")
            .filter(|h| unread > 0 && crate::send::tmux_has_session(h).unwrap_or(false))
        else {
            return Ok(WakePlan::Never);
        };
        let claude = field("client") == Some("claude");
        let turn = field("turn");
        if turn == Some("running") {
            return Ok(WakePlan::TurnEnd);
        }
        let status = field("status").and_then(parse_reported_status);
        let idle = idle_override
            .unwrap_or_else(|| list_panes_all().get(host).and_then(|p| p.idle_seconds));
        let pane_input = pane_tail(host).is_some_and(|t| pane_tail_looks_like_input(&t));
        let plan = wake_plan(claude, turn, status, pane_input, idle, quiet);
        if plan == WakePlan::Now {
            // Only Claude's UserPromptSubmit hook injects bodies (a `turn` proves its hooks).
            let injected = claude && turn.is_some();
            send_to_session(host, &wake_text(unread, injected), true)
                .map_err(|e| io::Error::other(e.to_string()))?;
        }
        Ok(plan)
    })
    .map_err(MsgError::from)
}

/// `cdamux msg wake <id>`: retry [`wake_once`] until it types the wake line or the inbox is
/// read; mid-turn it polls every `quiet` seconds, so a line that missed the `Stop` hook
/// still gets a wake once the turn ended. One waker per agent (a held lock); a waker that
/// finds the lock taken exits, and the holder re-checks the inbox after releasing it, so a
/// message appended meanwhile is not left without a waker.
pub fn run_waker(agent_id: &str, quiet: u32) -> Result<(), MsgError> {
    let lock = inbox_sibling(agent_id, "waker")?;
    let start = Instant::now();
    loop {
        let Some(held) = try_entity_lock(&lock)? else {
            return Ok(());
        };
        // Inbox length, not unread count: an ack plus an arrival leave the count unchanged.
        let mut seen = 0;
        while start.elapsed() < WAKER_MAX {
            seen = inbox_len(agent_id)?;
            match wake_once(agent_id, quiet, None) {
                Ok(WakePlan::After(s)) => std::thread::sleep(Duration::from_secs(s.into())),
                Ok(WakePlan::TurnEnd) => std::thread::sleep(Duration::from_secs(quiet.into())),
                _ => break,
            }
        }
        drop(held);
        if start.elapsed() >= WAKER_MAX || inbox_len(agent_id)? <= seen {
            return Ok(());
        }
    }
}

/// Format a short digest for hooks (one line per message, capped).
pub fn format_unread_digest(msgs: &[Message]) -> String {
    if msgs.is_empty() {
        return String::new();
    }
    let mut lines = vec![format!(
        "cdamux: {} unread message(s) — run: cdamux msg read --ack",
        msgs.len()
    )];
    for m in msgs.iter().take(5) {
        let preview: String = m.text.chars().take(80).collect();
        lines.push(format!("- [{}] from {}: {}", m.kind, m.from, preview));
    }
    if msgs.len() > 5 {
        lines.push(format!("- …and {} more", msgs.len() - 5));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::state::{initial_agent_value, state_set};

    fn with_temp_home<F: FnOnce(&Path)>(f: F) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn append_read_ack_round_trip() {
        with_temp_home(|home| {
            state_set(
                StateKind::Agents,
                "a-orch1",
                &initial_agent_value("a-orch1", "claude", "c-orch", "orchestrator", None),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-work1",
                &initial_agent_value("a-work1", "claude", "c-w1", "worker", Some("a-orch1")),
            )
            .unwrap();

            let msg = Message {
                id: "m-test1".into(),
                ts: 1,
                from: "a-work1".into(),
                to: "a-orch1".into(),
                kind: "report".into(),
                text: "done".into(),
                r#ref: Some("/tmp/r.json".into()),
            };
            append_message(&msg).unwrap();
            assert!(home.join("inbox/a-orch1.jsonl").is_file());

            let unread = read_messages("a-orch1", false).unwrap();
            assert_eq!(unread.len(), 1);
            assert_eq!(unread[0].text, "done");
            assert_eq!(unread[0].r#ref.as_deref(), Some("/tmp/r.json"));

            let again = read_messages("a-orch1", true).unwrap();
            assert_eq!(again.len(), 1);
            assert!(read_messages("a-orch1", false).unwrap().is_empty());
            assert_eq!(
                fs::read_to_string(home.join("inbox/a-orch1.cursor"))
                    .unwrap()
                    .trim()
                    .parse::<u64>()
                    .unwrap(),
                home.join("inbox/a-orch1.jsonl").metadata().unwrap().len()
            );
        });
    }

    fn test_msg(id: String, to: &str) -> Message {
        Message {
            id,
            ts: 1,
            from: "a-x".into(),
            to: to.into(),
            kind: "report".into(),
            text: "t".repeat(200),
            r#ref: None,
        }
    }

    #[test]
    fn concurrent_appends_keep_every_line() {
        with_temp_home(|_| {
            const THREADS: usize = 8;
            const PER: usize = 200;
            std::thread::scope(|s| {
                for t in 0..THREADS {
                    s.spawn(move || {
                        for i in 0..PER {
                            append_message(&test_msg(format!("m-{t}-{i}"), "a-conc1")).unwrap();
                        }
                    });
                }
            });
            assert_eq!(read_inbox_all("a-conc1").unwrap().len(), THREADS * PER);
        });
    }

    #[test]
    fn ack_stops_before_partial_line() {
        with_temp_home(|home| {
            append_message(&test_msg("m-p1".into(), "a-part1")).unwrap();
            append_message(&test_msg("m-p2".into(), "a-part1")).unwrap();
            let inbox = home.join("inbox/a-part1.jsonl");
            let two_lines = inbox.metadata().unwrap().len();
            let third = serde_json::to_string(&test_msg("m-p3".into(), "a-part1")).unwrap();
            let (head, tail) = third.split_at(10);
            OpenOptions::new()
                .append(true)
                .open(&inbox)
                .unwrap()
                .write_all(head.as_bytes())
                .unwrap();

            let got = read_messages("a-part1", true).unwrap();
            assert_eq!(got.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m-p1", "m-p2"]);
            let cursor: u64 = fs::read_to_string(home.join("inbox/a-part1.cursor"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert_eq!(cursor, two_lines);

            OpenOptions::new()
                .append(true)
                .open(&inbox)
                .unwrap()
                .write_all(format!("{tail}\n").as_bytes())
                .unwrap();
            let got = read_messages("a-part1", true).unwrap();
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].id, "m-p3");
            assert!(read_messages("a-part1", false).unwrap().is_empty());
        });
    }

    #[test]
    fn rejects_oversized_line() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-big1",
                &initial_agent_value("a-big1", "claude", "c-b", "orchestrator", None),
            )
            .unwrap();
            let big = "x".repeat(MAX_MSG_BYTES);
            let msg = Message {
                id: "m-big01".into(),
                ts: 1,
                from: "a-x".into(),
                to: "a-big1".into(),
                kind: "report".into(),
                text: big,
                r#ref: None,
            };
            assert_eq!(append_message(&msg), Err(MsgError::TooLarge));
        });
    }

    #[test]
    fn inbox_badges_unread_and_pending_ask() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-orchb",
                &initial_agent_value("a-orchb", "claude", "c-ob", "orchestrator", None),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-wbad",
                &initial_agent_value("a-wbad", "claude", "c-wb", "worker", Some("a-orchb")),
            )
            .unwrap();
            assert_eq!(inbox_badges("a-orchb"), (0, false));
            append_message(&Message {
                id: "m-u1".into(),
                ts: 1,
                from: "a-wbad".into(),
                to: "a-orchb".into(),
                kind: "ask".into(),
                text: "pick one?".into(),
                r#ref: None,
            })
            .unwrap();
            assert_eq!(inbox_badges("a-orchb"), (1, false));
            assert_eq!(inbox_badges("a-wbad"), (0, true));
            assert!(has_pending_ask("a-wbad", "a-orchb"));
            reply_to_ask("a-wbad", "option A").unwrap();
            assert!(!has_pending_ask("a-wbad", "a-orchb"));
            assert_eq!(inbox_badges("a-wbad"), (1, false));
            // Orch still has the ask unread until ack; badge is unread only.
            assert_eq!(inbox_badges("a-orchb"), (1, false));
        });
    }

    #[test]
    fn ask_sets_sender_blocked_answer_clears() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-orch2",
                &initial_agent_value("a-orch2", "claude", "c-o2", "orchestrator", None),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-ask1",
                &initial_agent_value("a-ask1", "claude", "c-a1", "worker", Some("a-orch2")),
            )
            .unwrap();

            // No live tmux: worker paste fails — exercise status only via apply + append.
            append_message(&Message {
                id: "m-ask01".into(),
                ts: 1,
                from: "a-ask1".into(),
                to: "a-orch2".into(),
                kind: "ask".into(),
                text: "which schema?".into(),
                r#ref: None,
            })
            .unwrap();
            apply_kind_status("ask", "a-ask1", "a-orch2").unwrap();
            assert_eq!(
                state_get(StateKind::Agents, "a-ask1").unwrap().unwrap()["status"],
                "blocked"
            );

            append_message(&Message {
                id: "m-ans01".into(),
                ts: 2,
                from: "a-orch2".into(),
                to: "a-ask1".into(),
                kind: "answer".into(),
                text: "use v2".into(),
                r#ref: None,
            })
            .unwrap();
            apply_kind_status("answer", "a-orch2", "a-ask1").unwrap();
            assert_eq!(
                state_get(StateKind::Agents, "a-ask1").unwrap().unwrap()["status"],
                "idle"
            );

            state_append(
                StateKind::Agents,
                "a-ask1",
                &json!({ "status": "working" }),
            )
            .unwrap();
            apply_kind_status("answer", "a-orch2", "a-ask1").unwrap();
            assert_eq!(
                state_get(StateKind::Agents, "a-ask1").unwrap().unwrap()["status"],
                "working"
            );
        });
    }

    /// Q21 truth table: when the wake line may be typed.
    #[test]
    fn wake_plan_truth_table() {
        use SessionState::*;
        use WakePlan::*;
        type Row = (bool, Option<&'static str>, Option<SessionState>, bool, Option<u32>, WakePlan);
        let q = 60;
        let rows: &[Row] = &[
            // Claude at rest (turn ended after its Stop hook): quiet window only.
            (true, Some("ended"), Some(Idle), false, Some(60), Now),
            (true, Some("ended"), Some(Idle), false, Some(4), After(56)), // answer inside the window
            (true, Some("ended"), Some(Blocked), false, Some(90), Now), // relay to a blocked worker
            (true, Some("ended"), Some(Done), false, Some(90), Now),
            (true, Some("ended"), Some(Idle), false, None, After(60)),
            (true, Some("ended"), Some(NeedsInput), false, Some(900), After(60)),
            (true, Some("ended"), Some(Working), false, Some(900), After(60)),
            // Claude mid-turn: the Stop hook delivers, whatever the status or pane (dialog).
            (true, Some("running"), Some(Working), false, Some(900), TurnEnd),
            (true, Some("running"), Some(NeedsInput), true, Some(900), TurnEnd),
            (true, Some("running"), Some(Blocked), false, Some(900), TurnEnd),
            // Claude with hooks but before its first prompt (startup dialogs): wait.
            (true, None, Some(Idle), false, Some(900), After(60)),
            // The CLI exited: the pane may be a shell.
            (true, Some("exited"), Some(Idle), false, Some(900), Never),
            (false, Some("exited"), Some(Idle), false, Some(900), Never),
            // No reliable turn signal (Claude without hooks, Cursor, codex, opencode):
            // idle / no status and a pane not waiting for an answer.
            (true, None, None, false, Some(60), Now),
            (false, None, Some(Idle), false, Some(61), Now),
            (false, Some("ended"), Some(Idle), false, Some(61), Now),
            (false, Some("ended"), Some(Working), false, Some(900), After(60)), // working still holds
            // A running turn is TurnEnd for every client that wrote `turn` (Cursor dialogs).
            (false, Some("running"), Some(Idle), false, Some(900), TurnEnd),
            (false, Some("running"), None, false, Some(900), TurnEnd),
            (false, Some("running"), Some(Done), false, Some(900), TurnEnd),
            (false, None, None, true, Some(900), After(60)), // pane looks like a prompt
            (false, None, Some(NeedsInput), false, Some(900), After(60)),
            (false, Some("ended"), Some(NeedsInput), false, Some(900), After(60)),
            // No turn: sticky done/error on a live pane is at rest like idle; blocked stays inbox.
            (false, None, Some(Blocked), false, Some(900), Never),
            (false, None, Some(Done), false, Some(900), Now),
            (false, None, Some(Error), false, Some(900), Now),
            (false, Some("ended"), Some(Done), false, Some(900), Now),
            (false, Some("ended"), Some(Error), false, Some(900), Now),
            (true, None, Some(Error), false, Some(900), After(60)),
        ];
        for &(claude, turn, status, pane_input, idle, want) in rows {
            let got = wake_plan(claude, turn, status, pane_input, idle, q);
            assert_eq!(got, want, "claude={claude} turn={turn:?} status={status:?} input={pane_input} idle={idle:?}");
    }
    }

    #[test]
    fn say_paste_requires_finished_turn_and_quiet() {
        for s in [SessionState::Idle, SessionState::Done, SessionState::Error] {
            assert!(should_paste(Some(s), Some(10), 10));
            assert!(!should_paste(Some(s), Some(5), 10));
        }
        for s in [SessionState::Working, SessionState::NeedsInput, SessionState::Blocked] {
            assert!(!should_paste(Some(s), Some(60), 10));
        }
        assert!(should_paste(None, Some(60), 10));
        assert!(!should_paste(None, Some(5), 10));
        assert!(!should_paste(Some(SessionState::Idle), None, 10));
    }

    #[test]
    fn wake_text_is_fixed_and_counts() {
        assert_eq!(wake_text(1, true), "📨 1 message");
        assert_eq!(wake_text(3, true), "📨 3 messages");
        assert_eq!(wake_text(1, false), "📨 1 message — cdamux msg read --ack");
    }

    #[test]
    fn delivery_words() {
        let words: Vec<_> = [Delivery::Woken, Delivery::Scheduled, Delivery::TurnEnd, Delivery::Inbox]
            .iter()
            .map(|d| d.as_str())
            .collect();
        assert_eq!(words, ["woken", "scheduled", "turn-end", "inbox"]);
    }

    #[test]
    fn msg_ids_are_time_prefixed_and_distinct() {
        assert_eq!(base36_secs(1_704_067_200), "000000");
        assert_eq!(base36_secs(1_704_067_200 + 36), "000010");
        assert!(base36_secs(now_ts()) > base36_secs(now_ts() - 86_400));
        let ids: std::collections::HashSet<_> = (0..50).filter_map(|_| generate_msg_id()).collect();
        assert_eq!(ids.len(), 50);
        assert!(ids.iter().all(|id| id.len() == 13 && id.starts_with("m-") && valid_state_id(id)));
    }

    #[test]
    fn read_max_bytes_acks_only_what_it_returns() {
        with_temp_home(|_| {
            let mut ends = vec![];
            for i in 0..3 {
                ends.push(append_message(&test_msg(format!("m-mx{i}"), "a-mx")).unwrap() as usize);
            }
            // Budget fits the first line only.
            let got = read_messages_max("a-mx", true, Some(ends[0] + 1)).unwrap();
            assert_eq!(got.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m-mx0"]);
            // A budget smaller than one line still returns one (never stuck).
            assert_eq!(read_messages_max("a-mx", true, Some(1)).unwrap()[0].id, "m-mx1");
            assert_eq!(read_messages("a-mx", true).unwrap()[0].id, "m-mx2");
            assert_eq!(unread_count("a-mx").unwrap(), 0);
        });
    }

    #[test]
    fn msg_send_refuses_control_characters() {
        with_temp_home(|_| {
            state_set(StateKind::Agents, "a-cc1", &json!({"id": "a-cc1", "role": "worker"})).unwrap();
            for bad in ["\x1b[31mred", "a\rb", "  "] {
                let e = msg_send("a-x", "a-cc1", "relay", bad, None, Some(None)).unwrap_err();
                assert!(matches!(e, MsgError::Invalid(_)), "{bad:?}: {e}");
            }
            assert!(msg_send("a-x", "a-cc1", "relay", "two\nlines\tok", None, Some(None)).is_ok());
        });
    }

    #[test]
    fn done_and_error_set_sender_status_sticky() {
        with_temp_home(|_| {
            let orch = json!({
                "id": "a-orchd", "client": "claude", "host": "missing-session",
                "role": "orchestrator", "status": "working"
            });
            state_set(StateKind::Agents, "a-orchd", &orch).unwrap();
            let status =
                |id: &str| state_get(StateKind::Agents, id).unwrap().unwrap()["status"].clone();
            for (id, kind, before, after) in [
                ("a-wd1", "done", "working", "done"),
                ("a-wd2", "error", "idle", "error"),
                // orch.sh sends a blocked worker as kind=error: blocked stays.
                ("a-wd3", "error", "blocked", "blocked"),
                // A worker that asked, then finished anyway: its own done wins.
                ("a-wd4", "done", "blocked", "done"),
                ("a-wd5", "done", "error", "error"),
            ] {
                state_set(
                    StateKind::Agents,
                    id,
                    &json!({ "id": id, "role": "worker", "parent": "a-orchd", "status": before }),
                )
                .unwrap();
                msg_send(id, "a-orchd", kind, "x", None, Some(None)).unwrap();
                assert_eq!(status(id), after, "{id}");
            }
            assert_eq!(status("a-orchd"), "working");
        });
    }

    #[test]
    fn resolve_by_host_name() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-host1",
                &initial_agent_value("a-host1", "cursor", "fleet-demo", "orchestrator", None),
            )
            .unwrap();
            assert_eq!(resolve_agent_id("fleet-demo").unwrap(), "a-host1");
            assert_eq!(resolve_agent_id("a-host1").unwrap(), "a-host1");
        });
    }

    #[test]
    fn resolve_name_ambiguous_unless_caller_child() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-o1",
                &initial_agent_value("a-o1", "claude", "c-o1", "orchestrator", None),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-o2",
                &initial_agent_value("a-o2", "claude", "c-o2", "orchestrator", None),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-w1",
                &json!({
                    "id": "a-w1",
                    "client": "claude",
                    "host": "c-w1",
                    "role": "worker",
                    "name": "api",
                    "parent": "a-o1",
                    "run": "r-1",
                    "status": "idle"
                }),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-w2",
                &json!({
                    "id": "a-w2",
                    "client": "claude",
                    "host": "c-w2",
                    "role": "worker",
                    "name": "api",
                    "parent": "a-o2",
                    "run": "r-2",
                    "status": "idle"
                }),
            )
            .unwrap();

            std::env::remove_var("CDAMUX_AGENT_ID");
            assert_eq!(
                resolve_agent_id("api"),
                Err(MsgError::Ambiguous("api".into()))
            );

            std::env::set_var("CDAMUX_AGENT_ID", "a-o1");
            assert_eq!(resolve_agent_id("api").unwrap(), "a-w1");
            std::env::set_var("CDAMUX_AGENT_ID", "a-o2");
            assert_eq!(resolve_agent_id("api").unwrap(), "a-w2");
            std::env::remove_var("CDAMUX_AGENT_ID");
        });
    }

    #[test]
    fn msg_send_delivery_without_live_pane() {
        with_temp_home(|_| {
            let to = |id: &str, extra: serde_json::Value| {
                let mut v = json!({"id": id, "client": "claude", "role": "worker", "parent": "a-o"});
                v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
                state_set(StateKind::Agents, id, &v).unwrap();
            };
            let send = |id: &str, idle| msg_send("a-o", id, "relay", "hi", None, idle).unwrap();
            // No host, or a gone pane, whatever the turn: inbox only, no waker.
            // (Live panes: tests/delivery_tmux.rs in cdamux-cli.)
            to("a-nh", json!({"status": "idle"}));
            assert_eq!(send("a-nh", Some(Some(900))).delivery, Delivery::Inbox);
            to("a-mt", json!({"host": "missing-session", "status": "blocked", "turn": "running"}));
            assert_eq!(send("a-mt", Some(Some(900))).delivery, Delivery::Inbox);
            to("a-nq", json!({"host": "missing-session", "status": "idle", "turn": "ended"}));
            assert_eq!(send("a-nq", Some(Some(3))).delivery, Delivery::Inbox);
            assert_eq!(read_messages("a-nq", false).unwrap().len(), 1);
        });
    }

    #[test]
    fn relay_and_answer_lift_blocked_and_answer_the_ask() {
        with_temp_home(|_| {
            for id in ["a-ob", "a-wb"] {
                let role = if id == "a-ob" { "orchestrator" } else { "worker" };
                state_set(StateKind::Agents, id, &json!({"id": id, "role": role, "parent": "a-ob"}))
                    .unwrap();
            }
            let status = || state_get(StateKind::Agents, "a-wb").unwrap().unwrap()["status"].clone();
            msg_send("a-wb", "a-ob", "ask", "which db?", None, Some(None)).unwrap();
            assert_eq!(status(), "blocked");
            assert!(has_pending_ask("a-wb", "a-ob"));
            // A plain report does not answer.
            msg_send("a-ob", "a-wb", "report", "fyi", None, Some(None)).unwrap();
            assert_eq!(status(), "blocked");
            msg_send("a-ob", "a-wb", "relay", "postgres", None, Some(None)).unwrap();
            assert_eq!(status(), "idle");
            assert!(!has_pending_ask("a-wb", "a-ob"));
        });
    }

    #[test]
    fn relay_and_answer_lift_done_and_error_to_idle() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-od",
                &json!({"id": "a-od", "role": "orchestrator"}),
            )
            .unwrap();
            for (id, before, kind) in [("a-wdone", "done", "relay"), ("a-werr", "error", "answer")] {
                state_set(
                    StateKind::Agents,
                    id,
                    &json!({"id": id, "role": "worker", "parent": "a-od", "status": before}),
                )
                .unwrap();
                msg_send("a-od", id, kind, "continue", None, Some(None)).unwrap();
                assert_eq!(
                    state_get(StateKind::Agents, id).unwrap().unwrap()["status"],
                    "idle",
                    "{id}"
                );
            }
        });
    }

    #[test]
    fn digest_format() {
        let msgs = vec![Message {
            id: "m-1".into(),
            ts: 1,
            from: "a-w".into(),
            to: "a-o".into(),
            kind: "done".into(),
            text: "ok".into(),
            r#ref: None,
        }];
        let d = format_unread_digest(&msgs);
        assert!(d.contains("1 unread"));
        assert!(d.contains("cdamux msg read --ack"));
        assert!(d.contains("[done] from a-w: ok"));
    }
}
