//! Inbox messages under `$CDAMUX_HOME/inbox/` (ADR-011 §4).
//! JSONL ≤ 2 KB per line; ack cursor is `<agent-id>.cursor` (byte offset).

use crate::agent_status::load_reported_status;
use crate::paths::cdamux_state_dir;
use crate::send::send_to_session;
use crate::session::SessionState;
use crate::session_registry;
use crate::state::{state_append, state_get, valid_state_id, StateKind};
use crate::tmux::list_panes_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    use std::io::Read as _;
    const ALPH: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut b = [0u8; 5];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut b).ok()?;
    let s: String = b
        .iter()
        .map(|x| ALPH[(*x as usize) % ALPH.len()] as char)
        .collect();
    Some(format!("m-{s}"))
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
    // Scan agents for matching host (session name).
    if let Some(dir) = cdamux_state_dir("agents") {
        if let Ok(entries) = fs::read_dir(dir) {
            for ent in entries.flatten() {
                let path = ent.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(v): Result<Value, _> = serde_json::from_str(&text) else {
                    continue;
                };
                if v.get("host").and_then(|h| h.as_str()) == Some(to) {
                    if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
                        return Ok(id.to_string());
                    }
                }
            }
        }
    }
    Err(MsgError::NotFound(to.to_string()))
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

pub fn nudge_text(unread: usize) -> String {
    format!("📨 {unread} messages — cdamux msg read")
}

fn read_cursor(path: &Path) -> io::Result<u64> {
    match fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
        Ok(s) => Ok(s.trim().parse::<u64>().unwrap_or(0)),
    }
}

fn write_cursor(path: &Path, offset: u64) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("cursor.tmp.{}", std::process::id()));
    fs::write(&tmp, format!("{offset}\n"))?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Append one message line (O_APPEND). Rejects lines over [`MAX_MSG_BYTES`].
pub fn append_message(msg: &Message) -> Result<(), MsgError> {
    let line = serde_json::to_string(msg).map_err(|e| MsgError::Io(e.to_string()))?;
    if line.len() > MAX_MSG_BYTES {
        return Err(MsgError::TooLarge);
    }
    let path = inbox_path(&msg.to)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// Read unread messages after the ack cursor. When `ack`, advance the cursor to EOF.
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
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    let mut out = Vec::new();
    for line in buf.lines() {
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
        write_cursor(&cur_path, len)?;
    }
    Ok(out)
}

/// Count unread messages (no ack).
pub fn unread_count(agent_id: &str) -> Result<usize, MsgError> {
    Ok(read_messages(agent_id, false)?.len())
}

fn apply_kind_status(kind: &str, from: &str, to: &str) -> Result<(), MsgError> {
    match kind {
        "ask" => {
            state_append(StateKind::Agents, from, &json!({ "status": "blocked" }))
                .map_err(|e| MsgError::Io(e.to_string()))?;
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

    append_message(&msg)?;
    apply_kind_status(kind, from, &to)?;

    let (role, host) = agent_role_and_host(&to)?;
    let delivery = delivery_mode_for_role(&role);
    let mut pasted = false;
    let mut nudged = false;

    // Inbox append is the source of truth; pane paste/nudge are best-effort.
    match delivery {
        DeliveryMode::WorkerPaste => {
            if let Some(session) = host.as_deref() {
                if send_to_session(session, text, true).is_ok() {
                    pasted = true;
                }
            }
        }
        DeliveryMode::OrchNudge => {
            let status = load_reported_status(&to);
            let idle = match idle_override {
                Some(v) => v,
                None => host
                    .as_ref()
                    .and_then(|h| list_panes_all().get(h).and_then(|p| p.idle_seconds)),
            };
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
