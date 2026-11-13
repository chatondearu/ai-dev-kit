//! Inbox messages under `$CDAMUX_HOME/inbox/` (ADR-011 §4).
//! JSONL ≤ 2 KB per line; ack cursor is `<agent-id>.cursor` (byte offset).

use crate::agent_status::{load_reported_status, may_overwrite, parse_reported_status};
use crate::paths::cdamux_state_dir;
use crate::send::send_to_session;
use crate::session::SessionState;
use crate::session_registry;
use crate::state::{
    random_id, state_append, state_get, state_list, valid_state_id, write_atomic, StateKind,
};
use crate::tmux::list_panes_all;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Max serialised JSON line size (ADR-011 §4).
pub const MAX_MSG_BYTES: usize = 2048;

/// Idle pane quiet window before an orchestrator nudge (seconds).
pub const DEFAULT_NUDGE_QUIET_SECS: u32 = 10;

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

/// How delivery should treat the target after the inbox append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Worker: paste into its pane.
    WorkerPaste,
    /// Orchestrator / free agent: optional idle nudge.
    OrchNudge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendOutcome {
    pub message: Message,
    pub delivery: DeliveryMode,
    pub nudged: bool,
    pub pasted: bool,
}

impl SendOutcome {
    /// What happened beyond the inbox append: `pasted`, `nudged` or `queued` (inbox only).
    pub fn delivery_word(&self) -> &'static str {
        if self.pasted {
            "pasted"
        } else if self.nudged {
            "nudged"
        } else {
            "queued"
        }
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

fn inbox_path(agent_id: &str) -> io::Result<PathBuf> {
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

/// Mint `m-xxxxx` (5 lowercase alphanumeric). `None` without `/dev/urandom`.
pub fn generate_msg_id() -> Option<String> {
    random_id("m")
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

fn agent_role_and_host(id: &str) -> Result<(String, Option<String>), MsgError> {
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

pub fn delivery_mode_for_role(role: &str) -> DeliveryMode {
    if role == "worker" {
        DeliveryMode::WorkerPaste
    } else {
        DeliveryMode::OrchNudge
    }
}

/// True when status is `idle` and the pane has been quiet for `quiet_secs`.
pub fn should_nudge(
    status: Option<SessionState>,
    idle_seconds: Option<u32>,
    quiet_secs: u32,
) -> bool {
    status == Some(SessionState::Idle)
        && idle_seconds.is_some_and(|s| s >= quiet_secs.max(1))
}

/// Paste into a worker pane only after a finished turn (`idle`, or sticky `done` /
/// `error`; no status = no hooks, so the quiet pane alone decides) and the nudge's quiet
/// window: the paste ends with Enter, so on a busy, a just-active or a permission-prompt
/// pane it would merge with (and submit) what the user is typing, or answer the prompt.
/// Otherwise the message stays in the inbox and the status hook's digest delivers it at
/// the worker's next prompt submit or turn end.
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

pub fn nudge_text(unread: usize) -> String {
    format!("📨 {unread} messages — cdamux msg read --ack")
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

/// Move `agent_id`'s ack cursor from `from` to `to` only when it sits at `from`, so a
/// pasted line is marked read without skipping older unread lines.
fn move_cursor_if_at(agent_id: &str, from: u64, to: u64) -> bool {
    let Ok(path) = cursor_path(agent_id) else {
        return false;
    };
    read_cursor(&path).ok() == Some(from) && write_cursor(&path, to).is_ok()
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
        fs::create_dir_all(dir)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    // One write(2) so concurrent O_APPEND senders cannot interleave line and newline.
    f.write_all(format!("{line}\n").as_bytes())?;
    // O_APPEND leaves the fd offset at the end of this write, even with concurrent writers.
    Ok(f.stream_position()?)
}

/// Read unread messages after the ack cursor. When `ack`, advance the cursor past the
/// last complete line read; a trailing partial line (no newline yet) stays unread.
pub fn read_messages(agent_id: &str, ack: bool) -> Result<Vec<Message>, MsgError> {
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
    let consumed = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
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

/// True when `worker` sent an `ask` to `parent` with no later `answer` back.
pub fn has_pending_ask(worker_id: &str, parent_id: &str) -> bool {
    let Ok(parent_msgs) = read_inbox_all(parent_id) else {
        return false;
    };
    let Ok(worker_msgs) = read_inbox_all(worker_id) else {
        return false;
    };
    parent_msgs.iter().any(|ask| {
        ask.kind == "ask"
            && ask.from == worker_id
            && !worker_msgs.iter().any(|ans| {
                ans.kind == "answer" && ans.from == parent_id && ans.ts >= ask.ts
            })
    })
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
    msg_send(
        &parent,
        worker_id,
        "answer",
        text,
        None,
        DEFAULT_NUDGE_QUIET_SECS,
        None,
    )
}

fn apply_kind_status(kind: &str, from: &str, to: &str) -> Result<(), MsgError> {
    match kind {
        "ask" => {
            state_append(StateKind::Agents, from, &json!({ "status": "blocked" }))
                .map_err(|e| MsgError::Io(e.to_string()))?;
        }
        "done" | "error" => {
            // Sticky (agent_status::may_overwrite): orch.sh reports a `blocked` worker as
            // kind=error, and that `blocked` is kept.
            let rec =
                state_get(StateKind::Agents, from).map_err(|e| MsgError::Io(e.to_string()))?;
            if let Some(v) = rec {
                let cur = v
                    .get("status")
                    .and_then(|s| s.as_str())
                    .and_then(parse_reported_status);
                let next = if kind == "done" { SessionState::Done } else { SessionState::Error };
                if may_overwrite(cur, next) {
                    state_append(StateKind::Agents, from, &json!({ "status": kind }))
                        .map_err(|e| MsgError::Io(e.to_string()))?;
                }
            }
        }
        "answer" => {
            // Clear asker blocked → idle only; do not wipe other statuses.
            let blocked = state_get(StateKind::Agents, to)
                .map_err(|e| MsgError::Io(e.to_string()))?
                .and_then(|v| v.get("status").and_then(|s| s.as_str()).map(str::to_owned));
            if blocked.as_deref() == Some("blocked") {
                state_append(StateKind::Agents, to, &json!({ "status": "idle" }))
                    .map_err(|e| MsgError::Io(e.to_string()))?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Build, append, deliver (paste / nudge), and apply ask/answer status side-effects.
///
/// `quiet_secs` is injectable for tests (production: [`DEFAULT_NUDGE_QUIET_SECS`]).
/// `idle_override` injects pane idle seconds in tests (`None` → read from tmux).
pub fn msg_send(
    from: &str,
    to_raw: &str,
    kind: &str,
    text: &str,
    r#ref: Option<&str>,
    quiet_secs: u32,
    idle_override: Option<Option<u32>>,
) -> Result<SendOutcome, MsgError> {
    if !valid_kind(kind) {
        return Err(MsgError::Invalid(format!(
            "kind must be one of {}",
            KINDS.join("|")
        )));
    }
    if text.is_empty() {
        return Err(MsgError::Invalid("text must not be empty".into()));
    }
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

    let end = append_message(&msg)?;
    let start = end - (line.len() as u64 + 1);
    apply_kind_status(kind, from, &to)?;

    let (role, host) = agent_role_and_host(&to)?;
    let delivery = delivery_mode_for_role(&role);
    let mut pasted = false;
    let mut nudged = false;
    let status = load_reported_status(&to);
    let idle = match idle_override {
        Some(v) => v,
        None => host
            .as_ref()
            .and_then(|h| list_panes_all().get(h).and_then(|p| p.idle_seconds)),
    };

    // Inbox append is the source of truth; pane paste/nudge are best-effort.
    match delivery {
        DeliveryMode::WorkerPaste => {
            let session = host.as_deref().filter(|_| should_paste(status, idle, quiet_secs));
            if let Some(session) = session {
                // Ack before the paste: its prompt-submit hook must not digest this line again.
                let acked = move_cursor_if_at(&to, start, end);
                pasted = send_to_session(session, text, true).is_ok();
                if acked && !pasted {
                    move_cursor_if_at(&to, end, start);
                }
            }
        }
        DeliveryMode::OrchNudge => {
            if should_nudge(status, idle, quiet_secs) {
                if let Some(session) = host.as_deref() {
                    let n = unread_count(&to).unwrap_or(1).max(1);
                    if send_to_session(session, &nudge_text(n), true).is_ok() {
                        nudged = true;
                    }
                }
            }
        }
    }

    Ok(SendOutcome {
        message: msg,
        delivery,
        nudged,
        pasted,
    })
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

    #[test]
    fn nudge_requires_idle_and_quiet() {
        assert!(!should_nudge(Some(SessionState::Idle), Some(5), 10));
        assert!(should_nudge(Some(SessionState::Idle), Some(10), 10));
        assert!(!should_nudge(Some(SessionState::Working), Some(60), 10));
        assert!(!should_nudge(None, Some(60), 10));
    }

    #[test]
    fn paste_requires_finished_turn_and_quiet() {
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
    fn nudge_text_acks() {
        assert!(nudge_text(2).ends_with("cdamux msg read --ack"));
    }

    #[test]
    fn delivery_word_names_outcome() {
        let out = |pasted, nudged| SendOutcome {
            message: test_msg("m-dw".into(), "a-x"),
            delivery: DeliveryMode::WorkerPaste,
            nudged,
            pasted,
        };
        assert_eq!(out(true, false).delivery_word(), "pasted");
        assert_eq!(out(false, true).delivery_word(), "nudged");
        assert_eq!(out(false, false).delivery_word(), "queued");
    }

    #[test]
    fn cursor_moves_only_from_expected_offset() {
        with_temp_home(|_| {
            let a = append_message(&test_msg("m-c1".into(), "a-cur1")).unwrap();
            let b = append_message(&test_msg("m-c2".into(), "a-cur1")).unwrap();
            // Cursor at 0, line 2 starts at `a`: line 1 is still unread, keep it.
            assert!(!move_cursor_if_at("a-cur1", a, b));
            assert_eq!(unread_count("a-cur1").unwrap(), 2);
            assert!(move_cursor_if_at("a-cur1", 0, a));
            assert_eq!(read_messages("a-cur1", false).unwrap()[0].id, "m-c2");
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
            ] {
                state_set(
                    StateKind::Agents,
                    id,
                    &json!({ "id": id, "role": "worker", "parent": "a-orchd", "status": before }),
                )
                .unwrap();
                msg_send(id, "a-orchd", kind, "x", None, 10, Some(None)).unwrap();
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
    fn msg_send_orch_nudge_with_injectable_idle() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-from1",
                &initial_agent_value("a-from1", "claude", "c-f", "worker", None),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-to1",
                &json!({
                    "id": "a-to1",
                    "client": "claude",
                    "host": "missing-session",
                    "role": "orchestrator",
                    "status": "idle",
                    "parent": null,
                    "run": null
                }),
            )
            .unwrap();

            // Idle + quiet but no live session → append ok, nudge soft-fails.
            let out = msg_send(
                "a-from1",
                "a-to1",
                "report",
                "hi",
                None,
                10,
                Some(Some(15)),
            )
            .unwrap();
            assert!(!out.nudged);
            assert_eq!(read_messages("a-to1", false).unwrap().len(), 1);

            // Not quiet enough → append only, no nudge.
            let out = msg_send(
                "a-from1",
                "a-to1",
                "report",
                "again",
                None,
                10,
                Some(Some(3)),
            )
            .unwrap();
            assert!(!out.nudged);
            assert!(!out.pasted);
            assert_eq!(out.delivery, DeliveryMode::OrchNudge);
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
