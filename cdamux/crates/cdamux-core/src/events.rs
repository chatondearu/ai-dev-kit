//! Read-only events feed: append-only `$CDAMUX_HOME/events.jsonl` journal (ADR-011 §2).
//! Schema `v: 1` in `docs/events.md`. Writers are fail-open: a journal error never
//! breaks `msg send`, a state write or a hook. Readers never touch inbox cursors.

use crate::msg::Message;
use crate::paths::cdamux_home;
use crate::session_links::read_tail;
use crate::state::{state_append, state_get, with_entity_lock, StateKind};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const EVENTS_SCHEMA_VERSION: u64 = 1;
/// `reply` text cap (bytes, cut on a char boundary); longer text sets `truncated: true`.
pub const MAX_REPLY_BYTES: usize = 8 * 1024;
/// The live segment rotates to `events.1.jsonl` past this size; two segments are kept.
const SEGMENT_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Transcript bytes read at turn end to find the last turn's assistant text.
const REPLY_TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;
/// Bytes read from a segment end to find its last cursor (one line is ≤ ~9 KiB).
const LAST_CURSOR_TAIL_BYTES: u64 = 64 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EventsError {
    #[error("invalid cursor '{0}'")]
    BadCursor(String),
    #[error("cursor {since} expired: oldest retained item is {oldest}; replay with --since 0")]
    Expired { since: u64, oldest: u64 },
    #[error("cursor {since} is ahead of the journal (last item {last}); was it reset? replay with --since 0")]
    Ahead { since: u64, last: u64 },
    #[error("io: {0}")]
    Io(String),
}

impl From<io::Error> for EventsError {
    fn from(e: io::Error) -> Self {
        EventsError::Io(e.to_string())
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn journal_path() -> io::Result<PathBuf> {
    cdamux_home().map(|h| h.join("events.jsonl")).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "CDAMUX_HOME / XDG_STATE_HOME / HOME unset")
    })
}

fn rotated_path(live: &Path) -> PathBuf {
    live.with_file_name("events.1.jsonl")
}

fn line_cursor(v: &Value) -> Option<u64> {
    v.get("cursor")?.as_str()?.parse().ok()
}

/// Cursor of the last parseable line of `path` (`None`: missing or no item).
fn last_cursor(path: &Path) -> io::Result<Option<u64>> {
    let tail = match read_tail(path, LAST_CURSOR_TAIL_BYTES) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        r => r?,
    };
    Ok(tail
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok().as_ref().and_then(line_cursor)))
}

fn ends_without_newline(path: &Path) -> io::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = match std::fs::File::open(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        r => r?,
    };
    if f.metadata()?.len() == 0 {
        return Ok(false);
    }
    let mut last = [0u8; 1];
    f.seek(SeekFrom::End(-1))?;
    f.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// Append one item under the journal lock; returns its cursor. The cursor is a
/// sequence number kept across rotation (read back from the last line).
fn append_at(live: &Path, mut item: Map<String, Value>, segment_max: u64) -> io::Result<u64> {
    with_entity_lock(live, || {
        let old = rotated_path(live);
        let last = match last_cursor(live)? {
            Some(c) => c,
            None => last_cursor(&old)?.unwrap_or(0),
        };
        if std::fs::metadata(live).is_ok_and(|m| m.len() >= segment_max) {
            std::fs::rename(live, &old)?;
        }
        let cursor = last + 1;
        let mut obj = Map::new();
        obj.insert("v".into(), json!(EVENTS_SCHEMA_VERSION));
        obj.insert("cursor".into(), json!(cursor.to_string()));
        obj.append(&mut item);
        // A torn last line (failed write) stays its own unparseable line.
        let sep = if ends_without_newline(live)? { "\n" } else { "" };
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(live)?;
        f.write_all(format!("{sep}{}\n", Value::Object(obj)).as_bytes())?;
        Ok(cursor)
    })
}

/// Fail-open append of one item (`type`, `ts` and the type's fields); `false` when
/// the journal could not be written.
pub fn emit(item: Value) -> bool {
    let (Ok(live), Value::Object(map)) = (journal_path(), item) else {
        return false;
    };
    append_at(&live, map, SEGMENT_MAX_BYTES).is_ok()
}

/// `msg` item for a message appended to an orchestrator's inbox.
pub fn emit_msg(m: &Message) {
    let mut item = json!({
        "type": "msg", "ts": m.ts, "id": m.id, "from": m.from, "to": m.to,
        "kind": m.kind, "text": m.text,
    });
    if let Some(r) = &m.r#ref {
        item["ref"] = json!(r);
    }
    emit(item);
}

/// `status` item when an agent record's `status` changes (`record`: the merged record).
pub fn emit_status(agent: &str, status: &str, prev: Option<&str>, record: &Value) {
    let mut item = json!({
        "type": "status", "ts": now_ts(), "agent": agent, "status": status, "prev": prev,
    });
    for key in ["role", "parent"] {
        if let Some(v) = record.get(key).and_then(Value::as_str) {
            item[key] = json!(v);
        }
    }
    emit(item);
}

/// Lifecycle verbs of a `lifecycle` item (#398).
pub const LIFECYCLE_EVENTS: &[&str] = &["spawn", "finalize", "dismiss", "kill", "archive"];

/// `lifecycle` item: `event` is one of [`LIFECYCLE_EVENTS`] for agent `agent`
/// (`record`: its document, for `role` / `parent` / `host`; `None` when unknown).
pub fn emit_lifecycle(event: &str, agent: &str, record: Option<&Value>) {
    debug_assert!(LIFECYCLE_EVENTS.contains(&event));
    let mut item = json!({ "type": "lifecycle", "ts": now_ts(), "event": event, "agent": agent });
    for key in ["role", "parent", "host"] {
        if let Some(v) = record
            .and_then(|r| r.get(key))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            item[key] = json!(v);
        }
    }
    emit(item);
}

/// Every retained item with a cursor strictly after `since` (`0`: all retained).
/// Lock-free so a follower never stalls a hook's write: the live segment is read
/// before the rotated one, so a rotation in between duplicates items (merged by
/// cursor) instead of hiding them.
fn read_at(live: &Path, since: u64) -> Result<Vec<Value>, EventsError> {
    let mut items = BTreeMap::new();
    for p in [live.to_path_buf(), rotated_path(live)] {
        let bytes = match std::fs::read(&p) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            r => r?,
        };
        // A line still being written or torn by a failed write (even mid UTF-8
        // sequence) does not parse and is skipped; the rest stays readable.
        let text = String::from_utf8_lossy(&bytes);
        for v in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
            if let Some(c) = line_cursor(&v) {
                items.insert(c, v);
            }
        }
    }
    let oldest = items.keys().next().copied().unwrap_or(0);
    let last = items.keys().next_back().copied().unwrap_or(0);
    if since > last {
        return Err(EventsError::Ahead { since, last });
    }
    if since > 0 && oldest > since + 1 {
        return Err(EventsError::Expired { since, oldest });
    }
    Ok(items.into_iter().filter(|(c, _)| *c > since).map(|(_, v)| v).collect())
}

/// Parse a `--since` cursor (`0` replays every retained item).
pub fn parse_cursor(s: &str) -> Result<u64, EventsError> {
    s.trim().parse().map_err(|_| EventsError::BadCursor(s.to_string()))
}

pub fn read_events(since: u64) -> Result<Vec<Value>, EventsError> {
    read_at(&journal_path()?, since)
}

/// Lengths of both segments: unchanged means no new item since the last read.
pub fn journal_stamp() -> (u64, u64) {
    let len = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
    journal_path().map_or((0, 0), |live| (len(&rotated_path(&live)), len(&live)))
}

/// Feed filter: `agent` keeps that agent's items, its inbox traffic and its workers'
/// status; `orchestrators` keeps msg / reply / human and orchestrator status items.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub agent: Option<String>,
    pub orchestrators: bool,
}

impl EventFilter {
    pub fn matches(&self, v: &Value) -> bool {
        let field = |k: &str| v.get(k).and_then(Value::as_str);
        if let Some(id) = &self.agent {
            let id = Some(id.as_str());
            if ![field("agent"), field("to"), field("from"), field("parent")].contains(&id) {
                return false;
            }
        }
        !self.orchestrators
            || !matches!(field("type"), Some("status" | "lifecycle"))
            || field("role") == Some("orchestrator")
    }
}

/// Assistant text of the last turn of a Claude / Cursor JSONL transcript, and the
/// id of its last text entry. A turn starts at a user line holding text (tool
/// results do not count); tool calls, tool results and thinking are skipped.
pub fn last_turn_reply(jsonl: &str) -> Option<(String, String)> {
    let mut texts: Vec<String> = Vec::new();
    let mut turn_id = String::new();
    for line in jsonl.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["isSidechain"] == true || v["isMeta"] == true {
            continue;
        }
        let role = v["message"]["role"].as_str().or(v["role"].as_str()).unwrap_or_default();
        let text = match &v["message"]["content"] {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => continue,
        };
        let text = text.trim();
        match role {
            "user" if !text.is_empty() => {
                texts.clear();
                turn_id.clear();
            }
            "assistant" if !text.is_empty() => {
                texts.push(text.to_string());
                turn_id = v["uuid"]
                    .as_str()
                    .or(v["message"]["id"].as_str())
                    .map_or_else(|| format!("h-{:016x}", fnv1a(text)), str::to_string);
            }
            _ => {}
        }
    }
    (!texts.is_empty()).then(|| (texts.join("\n\n"), turn_id))
}

fn fnv1a(s: &str) -> u64 {
    s.bytes()
        .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Cap `text` at [`MAX_REPLY_BYTES`] on a char boundary; `true` when cut.
fn cap_text(text: &str) -> (String, bool) {
    if text.len() <= MAX_REPLY_BYTES {
        return (text.to_string(), false);
    }
    let mut end = MAX_REPLY_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

/// Journal text when an orchestrator turn ends with no assistant prose.
const TOOL_ONLY_REPLY: &str = "tool-only turn";

fn text_field<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload[key].as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// Per-turn id when a stop has no assistant prose. Cursor's `generation_id`
/// changes every user message, so each stop is one line and a retry of the
/// same stop matches `last_reply_turn`.
fn tool_only_turn_id(payload: &Value) -> String {
    if let Some(id) = text_field(payload, "generation_id") {
        return id.to_string();
    }
    if let Some(p) = payload["transcript_path"].as_str().filter(|s| !s.is_empty()) {
        if let Ok(tail) = read_tail(Path::new(p), REPLY_TRANSCRIPT_TAIL_BYTES) {
            let tail = tail.trim();
            if !tail.is_empty() {
                return format!("h-{:016x}", fnv1a(tail));
            }
        }
    }
    // shortcut: identical payloads and no transcript share one tool-only line.
    format!("h-{:016x}", fnv1a(&payload.to_string()))
}

/// Turn-end hook (`cdamux events reply`): emit one `reply` item for an orchestrator.
/// `payload` is the client hook JSON (`transcript_path`, else `last_assistant_message`).
/// No prose (empty last message, transcript missing or without assistant text) still
/// writes [`TOOL_ONLY_REPLY`]. A turn already recorded (same `turn_id` in
/// `last_reply_turn`) is skipped, including a real reply stored under the same
/// `generation_id`.
/// Returns whether an item was written.
pub fn record_reply(agent: &str, payload: &Value) -> io::Result<bool> {
    let Some(rec) = state_get(StateKind::Agents, agent)? else {
        return Ok(false);
    };
    if rec.get("role").and_then(Value::as_str) != Some("orchestrator") {
        return Ok(false);
    }
    let last_msg = text_field(payload, "last_assistant_message");
    let from_transcript = payload["transcript_path"]
        .as_str()
        .filter(|p| !p.is_empty())
        .and_then(|p| read_tail(Path::new(p), REPLY_TRANSCRIPT_TAIL_BYTES).ok())
        .and_then(|t| last_turn_reply(&t))
        // The transcript may lag the Stop hook: trust it only when it already ends
        // with the client's last assistant message.
        .filter(|(text, _)| last_msg.is_none_or(|m| text.ends_with(m)));
    let reply = from_transcript
        .or_else(|| last_msg.map(|t| (t.to_string(), format!("h-{:016x}", fnv1a(t)))));
    // generation_id is the turn key, so a tool-only retry dedupes a real reply.
    let (text, turn_id) = match reply {
        Some((text, turn_id)) => {
            let id = text_field(payload, "generation_id").unwrap_or(&turn_id);
            (text, id.to_string())
        }
        None => (TOOL_ONLY_REPLY.to_string(), tool_only_turn_id(payload)),
    };
    if rec.get("last_reply_turn").and_then(Value::as_str) == Some(turn_id.as_str()) {
        return Ok(false);
    }
    let (text, truncated) = cap_text(&text);
    let mut item = json!({
        "type": "reply", "ts": now_ts(), "agent": agent, "text": text, "turn_id": turn_id,
    });
    if truncated {
        item["truncated"] = json!(true);
    }
    // Unwritten item: leave the turn unrecorded so a later turn end can retry it.
    if !emit(item) {
        return Ok(false);
    }
    state_append(StateKind::Agents, agent, &json!({ "last_reply_turn": turn_id }))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::state::state_set;

    fn item(t: &str) -> Map<String, Value> {
        json!({ "type": t, "ts": 1 }).as_object().unwrap().clone()
    }

    #[test]
    fn cursor_replay_is_strictly_after() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("events.jsonl");
        assert_eq!(read_at(&live, 0).unwrap(), Vec::<Value>::new());
        for t in ["msg", "status", "reply"] {
            append_at(&live, item(t), SEGMENT_MAX_BYTES).unwrap();
        }
        let all = read_at(&live, 0).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|v| v["v"] == 1 && v["cursor"].is_string()));
        assert_eq!(all[2]["type"], "reply");
        let last = parse_cursor(all[2]["cursor"].as_str().unwrap()).unwrap();
        assert!(read_at(&live, last).unwrap().is_empty());
        append_at(&live, item("msg"), SEGMENT_MAX_BYTES).unwrap();
        assert_eq!(read_at(&live, last).unwrap().len(), 1);
        assert_eq!(read_at(&live, 99), Err(EventsError::Ahead { since: 99, last: 4 }));
        assert!(parse_cursor("x").is_err());
    }

    #[test]
    fn rotation_keeps_cursors_and_reports_expired() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("events.jsonl");
        // Tiny segments: every append past the first rotates.
        for _ in 0..5 {
            append_at(&live, item("msg"), 1).unwrap();
        }
        assert!(rotated_path(&live).exists());
        let kept: Vec<u64> =
            read_at(&live, 0).unwrap().iter().map(|v| line_cursor(v).unwrap()).collect();
        assert_eq!(kept, [4, 5], "cursors keep counting across rotation");
        assert_eq!(read_at(&live, 3).unwrap().len(), 2, "cursor just before oldest is fine");
        assert_eq!(read_at(&live, 2), Err(EventsError::Expired { since: 2, oldest: 4 }));
    }

    #[test]
    fn torn_line_neither_breaks_reads_nor_swallows_the_next_item() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("events.jsonl");
        append_at(&live, item("msg"), SEGMENT_MAX_BYTES).unwrap();
        // A failed write left half an item, cut inside a UTF-8 sequence, no newline.
        let mut f = std::fs::OpenOptions::new().append(true).open(&live).unwrap();
        f.write_all(b"{\"v\":1,\"cursor\":\"2\",\"text\":\"\xc3").unwrap();
        assert_eq!(read_at(&live, 0).unwrap().len(), 1);
        assert_eq!(append_at(&live, item("reply"), SEGMENT_MAX_BYTES).unwrap(), 2);
        let all = read_at(&live, 0).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1]["type"], "reply");
    }

    #[test]
    fn concurrent_writers_get_unique_contiguous_cursors() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("events.jsonl");
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let live = live.clone();
                // Small segments so rotation races with appends too.
                std::thread::spawn(move || {
                    (0..25).map(|_| append_at(&live, item("msg"), 4096).unwrap()).collect::<Vec<_>>()
                })
            })
            .collect();
        let mut got: Vec<u64> = threads.into_iter().flat_map(|t| t.join().unwrap()).collect();
        got.sort_unstable();
        assert_eq!(got, (1..=200).collect::<Vec<u64>>());
        let kept: Vec<u64> =
            read_at(&live, 0).unwrap().iter().map(|v| line_cursor(v).unwrap()).collect();
        let first = kept[0];
        assert_eq!(kept, (first..=200).collect::<Vec<u64>>(), "retained items are contiguous");
    }

    #[test]
    fn filter_by_agent_and_orchestrators() {
        let f = EventFilter { agent: Some("a-o".into()), orchestrators: false };
        assert!(f.matches(&json!({"type":"msg","from":"a-w","to":"a-o"})));
        assert!(f.matches(&json!({"type":"status","agent":"a-w","parent":"a-o"})));
        assert!(!f.matches(&json!({"type":"reply","agent":"a-x"})));
        let o = EventFilter { agent: None, orchestrators: true };
        assert!(o.matches(&json!({"type":"reply","agent":"a-x"})));
        assert!(o.matches(&json!({"type":"status","agent":"a-o","role":"orchestrator"})));
        assert!(!o.matches(&json!({"type":"status","agent":"a-w","role":"worker"})));
    }

    const TRANSCRIPT: &[&str] = &[
        r#"{"type":"user","uuid":"u0","message":{"role":"user","content":"old prompt"}}"#,
        r#"{"type":"assistant","uuid":"x0","message":{"role":"assistant","content":[{"type":"text","text":"old answer"}]}}"#,
        r#"{"type":"user","uuid":"u1","message":{"role":"user","content":"status?"}}"#,
        r#"{"type":"assistant","uuid":"x1","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hm"}]}}"#,
        r#"{"type":"assistant","uuid":"x2","message":{"role":"assistant","content":[{"type":"text","text":"Checking."},{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#,
        r#"{"type":"user","uuid":"u2","message":{"role":"user","content":[{"type":"tool_result","content":"secret tool output"}]}}"#,
        r#"{"type":"user","uuid":"u3","isMeta":true,"message":{"role":"user","content":"meta caveat"}}"#,
        r#"{"type":"assistant","uuid":"x3","message":{"role":"assistant","content":[{"type":"text","text":"Two workers done."}]}}"#,
    ];

    #[test]
    fn last_turn_reply_is_assistant_prose_only() {
        let (text, turn) = last_turn_reply(&TRANSCRIPT.join("\n")).unwrap();
        assert_eq!(text, "Checking.\n\nTwo workers done.");
        assert_eq!(turn, "x3");
        assert_eq!(last_turn_reply(TRANSCRIPT[2]), None, "no assistant text yet");
    }

    #[test]
    fn turn_end_writes_exactly_one_reply_for_orchestrators() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let tp = dir.path().join("t.jsonl");
        std::fs::write(&tp, TRANSCRIPT.join("\n")).unwrap();
        state_set(StateKind::Agents, "a-o", &json!({"id":"a-o","role":"orchestrator"})).unwrap();
        state_set(StateKind::Agents, "a-w", &json!({"id":"a-w","role":"worker"})).unwrap();
        let payload = json!({ "hook_event_name": "Stop", "transcript_path": tp });

        assert!(!record_reply("a-w", &payload).unwrap(), "workers emit no reply");
        assert!(record_reply("a-o", &payload).unwrap());
        assert!(!record_reply("a-o", &payload).unwrap(), "same turn twice → one item");
        let replies: Vec<Value> =
            read_events(0).unwrap().into_iter().filter(|v| v["type"] == "reply").collect();
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0]["agent"], "a-o");
        assert_eq!(replies[0]["text"], "Checking.\n\nTwo workers done.");
        assert!(!replies[0]["text"].as_str().unwrap().contains("secret"));

        // No transcript: fall back to the payload's last message; long text is capped.
        let long = "é".repeat(MAX_REPLY_BYTES);
        let p2 = json!({ "last_assistant_message": long });
        assert!(record_reply("a-o", &p2).unwrap());
        let last = read_events(0).unwrap().pop().unwrap();
        assert_eq!(last["truncated"], true);
        assert!(last["text"].as_str().unwrap().len() <= MAX_REPLY_BYTES);

        // Transcript lags the Stop hook (final message not flushed yet): use the payload's.
        let lag = json!({ "transcript_path": tp, "last_assistant_message": "All merged." });
        assert!(record_reply("a-o", &lag).unwrap());
        assert_eq!(read_events(0).unwrap().pop().unwrap()["text"], "All merged.");
        // Caught-up transcript: its full turn text wins over the last message alone.
        let synced = json!({ "transcript_path": tp, "last_assistant_message": "Two workers done." });
        assert!(record_reply("a-o", &synced).unwrap());
        assert_eq!(read_events(0).unwrap().pop().unwrap()["text"], "Checking.\n\nTwo workers done.");

        // Journal write fails: the turn stays unrecorded so a later turn end can retry.
        let before = state_get(StateKind::Agents, "a-o").unwrap().unwrap()["last_reply_turn"].clone();
        let live = dir.path().join("events.jsonl");
        std::fs::rename(&live, dir.path().join("saved.jsonl")).unwrap();
        std::fs::create_dir(&live).unwrap();
        assert!(!record_reply("a-o", &json!({ "last_assistant_message": "lost?" })).unwrap());
        assert_eq!(state_get(StateKind::Agents, "a-o").unwrap().unwrap()["last_reply_turn"], before);
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn tool_only_stop_writes_one_reply() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        state_set(StateKind::Agents, "a-o", &json!({"id":"a-o","role":"orchestrator"})).unwrap();
        state_set(StateKind::Agents, "a-w", &json!({"id":"a-w","role":"worker"})).unwrap();

        // Cursor stop: no assistant prose, no transcript.
        let stop = json!({
            "hook_event_name": "stop",
            "generation_id": "g-tool",
            "last_assistant_message": "",
            "status": "completed",
        });
        assert!(!record_reply("a-w", &stop).unwrap(), "workers emit no reply");
        assert!(record_reply("a-o", &stop).unwrap(), "tool-only stop writes a reply");
        assert!(!record_reply("a-o", &stop).unwrap(), "same turn twice → one item");
        let replies: Vec<Value> =
            read_events(0).unwrap().into_iter().filter(|v| v["type"] == "reply").collect();
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0]["agent"], "a-o");
        assert_eq!(replies[0]["text"], TOOL_ONLY_REPLY);
        assert_eq!(replies[0]["turn_id"], "g-tool");

        // Transcript of a tool call only: still no assistant prose.
        let tp = dir.path().join("tools.jsonl");
        std::fs::write(
            &tp,
            "{\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"name\":\"Bash\"}]}}\n",
        )
        .unwrap();
        let tools = json!({
            "hook_event_name": "stop",
            "generation_id": "g-bash",
            "last_assistant_message": "  ",
            "transcript_path": tp,
        });
        assert!(record_reply("a-o", &tools).unwrap(), "tool transcript still leaves a line");

        // Real prose for a generation wins; a later empty stop of that turn does not.
        let prose = json!({
            "hook_event_name": "stop",
            "generation_id": "g-prose",
            "last_assistant_message": "All merged.",
        });
        assert!(record_reply("a-o", &prose).unwrap());
        let again = json!({
            "hook_event_name": "stop",
            "generation_id": "g-prose",
            "last_assistant_message": "",
        });
        assert!(!record_reply("a-o", &again).unwrap(), "real reply already recorded");
        let texts: Vec<String> = read_events(0)
            .unwrap()
            .into_iter()
            .filter(|v| v["type"] == "reply")
            .map(|v| v["text"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            texts,
            vec![
                TOOL_ONLY_REPLY.to_string(),
                TOOL_ONLY_REPLY.to_string(),
                "All merged.".to_string(),
            ]
        );
        std::env::remove_var("CDAMUX_HOME");
    }
}
