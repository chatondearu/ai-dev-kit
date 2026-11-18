//! Human → orchestrator: `cdamux say` pastes a labelled prompt into an idle orchestrator
//! or queues it (`$CDAMUX_HOME/say/<id>.jsonl`, FIFO) until a turn ends; `msg answer`
//! answers a worker's ask on the human's behalf. Spec: `docs/say.md`.

use crate::agent_status::load_reported_status;
use crate::events::emit;
use crate::msg::{
    agent_role_and_host, ask_answered, find_message, generate_msg_id, msg_send, read_inbox_all,
    reply_to_ask, resolve_agent_id, should_paste, MsgError, DEFAULT_NUDGE_QUIET_SECS,
};
use crate::paths::cdamux_state_dir;
use crate::send::send_to_session;
use crate::session::SessionState;
use crate::state::{with_entity_lock, write_atomic};
use crate::tmux::list_panes_all;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SayOutcome {
    Delivered,
    Queued,
}

impl SayOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            SayOutcome::Delivered => "delivered",
            SayOutcome::Queued => "queued",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SayResult {
    pub id: String,
    /// Resolved orchestrator agent id.
    pub target: String,
    pub outcome: SayOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerResult {
    /// Id of the `answer` message the worker received.
    pub id: String,
    pub ask: String,
    pub worker: String,
    /// False when the `relay` copy to the orchestrator inbox failed (the answer went through).
    pub relayed: bool,
}

#[derive(Serialize, Deserialize)]
struct Queued {
    id: String,
    ts: u64,
    from: String,
    text: String,
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn queue_path(orch: &str) -> Result<PathBuf, MsgError> {
    cdamux_state_dir("say")
        .map(|d| d.join(format!("{orch}.jsonl")))
        .ok_or_else(|| MsgError::Io("CDAMUX_HOME / XDG_STATE_HOME / HOME unset".into()))
}

fn queue_push(orch: &str, q: &Queued) -> Result<(), MsgError> {
    let path = queue_path(orch)?;
    let line = format!("{}\n", serde_json::to_string(q).map_err(|e| MsgError::Io(e.to_string()))?);
    with_entity_lock(&path, || {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        f.write_all(line.as_bytes())
    })?;
    Ok(())
}

/// Paste the oldest queued prompt into `host` and drop it from the queue; returns its id.
/// The lock spans the paste and the drop only follows a successful paste: a hook killed
/// mid-way repeats a prompt rather than losing it, and two drainers never paste twice.
fn deliver_next(orch: &str, host: &str) -> Result<Option<String>, MsgError> {
    let path = queue_path(orch)?;
    Ok(with_entity_lock(&path, || {
        let text = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            r => r?,
        };
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let Some(q) = lines.find_map(|l| serde_json::from_str::<Queued>(l).ok()) else {
            return Ok(None);
        };
        if send_to_session(host, &format!("[{}] {}", q.from, q.text), true).is_err() {
            return Ok(None);
        }
        let rest: String = lines.map(|l| format!("{l}\n")).collect();
        write_atomic(&path, rest.as_bytes())?;
        Ok(Some(q.id))
    })?)
}

/// Non-empty and free of control characters (an ESC would drive the agent's TUI); only
/// the prompt text (`multiline`) may span lines.
fn check_text(what: &str, s: &str, multiline: bool) -> Result<(), MsgError> {
    if s.trim().is_empty() || s.chars().any(|c| c.is_control() && !(multiline && c == '\n')) {
        return Err(MsgError::Invalid(format!(
            "{what} must be non-empty text without control characters"
        )));
    }
    Ok(())
}

/// Deliver `text` to an orchestrator as a human prompt `[from] text`: pasted when it is
/// at rest (reported idle, or no status, and a quiet pane), else queued for its next turn
/// end ([`say_drain`]). A `needs_input` orchestrator waits on a permission prompt, where
/// the paste's Enter would answer it, so that is queued too. One `human` event per call.
///
/// `quiet_secs` / `idle_override` are injectable for tests like in [`msg_send`].
pub fn say(
    target: &str,
    from: &str,
    text: &str,
    quiet_secs: u32,
    idle_override: Option<Option<u32>>,
) -> Result<SayResult, MsgError> {
    check_text("--from", from, false)?;
    check_text("text", text, true)?;
    let from = from.trim();
    let to = resolve_agent_id(target)?;
    let (role, host) = agent_role_and_host(&to)?;
    if role != "orchestrator" {
        return Err(MsgError::Invalid(format!(
            "'{target}' is a {role}, not an orchestrator; use `cdamux msg send --to {target}`"
        )));
    }
    let id = generate_msg_id().ok_or_else(|| MsgError::Io("urandom unavailable".into()))?;
    queue_push(
        &to,
        &Queued { id: id.clone(), ts: now_ts(), from: from.into(), text: text.into() },
    )?;
    emit(json!({
        "type": "human", "ts": now_ts(), "id": id, "agent": to, "from": from, "text": text,
    }));

    let snapshot = || {
        let idle = idle_override.unwrap_or_else(|| {
            host.as_ref().and_then(|h| list_panes_all().get(h).and_then(|p| p.idle_seconds))
        });
        (load_reported_status(&to), idle)
    };
    let (mut status, mut idle) = snapshot();
    // Just after a turn end the pane still counts as active, and no later Stop would flush
    // the queue: wait out the quiet window once instead of queueing for good.
    if let (None | Some(SessionState::Idle), Some(s)) = (status, idle) {
        if s < quiet_secs {
            std::thread::sleep(Duration::from_secs(u64::from(quiet_secs - s)));
            (status, idle) = snapshot();
        }
    }
    let delivered = match host.as_deref().filter(|_| should_paste(status, idle, quiet_secs)) {
        Some(h) => deliver_next(&to, h)?.as_deref() == Some(id.as_str()),
        None => false,
    };
    Ok(SayResult {
        id,
        target: to,
        outcome: if delivered { SayOutcome::Delivered } else { SayOutcome::Queued },
    })
}

/// Turn-end hook (`cdamux msg drain`): paste the oldest queued prompt of an orchestrator
/// that reports `idle`. One prompt per turn end: the paste starts the next turn.
/// Returns the delivered prompt id.
pub fn say_drain(agent: &str) -> Result<Option<String>, MsgError> {
    let (role, host) = agent_role_and_host(agent)?;
    if role != "orchestrator" || load_reported_status(agent) != Some(SessionState::Idle) {
        return Ok(None);
    }
    host.map_or(Ok(None), |h| deliver_next(agent, &h))
}

/// Answer the worker behind an `ask` message on the human's behalf, through
/// [`reply_to_ask`], and leave a `relay` copy in the asked orchestrator's inbox.
/// Unknown id, a message that is not an ask, or an ask already answered: `Invalid` / `NotFound`.
pub fn answer_ask(ask_id: &str, from: &str, text: &str) -> Result<AnswerResult, MsgError> {
    check_text("--from", from, false)?;
    check_text("text", text, true)?;
    let from = from.trim();
    let ask = find_message(ask_id)?.ok_or_else(|| MsgError::NotFound(format!("ask {ask_id}")))?;
    if ask.kind != "ask" {
        return Err(MsgError::Invalid(format!("message {ask_id} is a '{}', not an ask", ask.kind)));
    }
    if ask_answered(&ask, &read_inbox_all(&ask.from)?) {
        return Err(MsgError::Invalid(format!("ask {ask_id} is already answered")));
    }
    let out = reply_to_ask(&ask.from, &format!("[{from}] {text}"))?;
    let relay = format!("[{from}] answered {}'s ask {ask_id}: {text}", ask.from);
    let relayed = msg_send(
        "human", &ask.to, "relay", &relay, Some(ask_id), DEFAULT_NUDGE_QUIET_SECS, None,
    )
    .is_ok();
    Ok(AnswerResult { id: out.message.id, ask: ask_id.into(), worker: ask.from, relayed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::state::{state_append, state_set, StateKind};
    use serde_json::Value;

    fn agent(id: &str, role: &str, parent: Option<&str>, status: &str) {
        let v = json!({"id": id, "role": role, "parent": parent, "status": status});
        state_set(StateKind::Agents, id, &v).unwrap();
    }

    fn with_home(f: impl FnOnce()) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f();
        std::env::remove_var("CDAMUX_HOME");
    }

    fn queue_len(orch: &str) -> usize {
        std::fs::read_to_string(queue_path(orch).unwrap()).map_or(0, |t| t.lines().count())
    }

    fn human_events() -> Vec<Value> {
        crate::events::read_events(0)
            .unwrap()
            .into_iter()
            .filter(|v| v["type"] == "human")
            .collect()
    }

    #[test]
    fn say_rejects_worker_unknown_and_empty() {
        with_home(|| {
            agent("a-so1", "orchestrator", None, "idle");
            agent("a-sw1", "worker", Some("a-so1"), "working");
            let e = say("a-sw1", "alice", "hi", 10, Some(Some(99))).unwrap_err();
            assert!(matches!(&e, MsgError::Invalid(m) if m.contains("msg send")), "{e}");
            assert!(matches!(say("a-nope", "alice", "hi", 10, None), Err(MsgError::NotFound(_))));
            assert!(matches!(say("a-so1", "alice", "  ", 10, None), Err(MsgError::Invalid(_))));
            assert!(matches!(say("a-so1", "", "hi", 10, None), Err(MsgError::Invalid(_))));
            assert!(matches!(say("a-so1", "a\nb", "hi", 10, None), Err(MsgError::Invalid(_))));
            assert!(matches!(say("a-so1", "alice", "\x1b[31m", 10, None), Err(MsgError::Invalid(_))));
            assert!(human_events().is_empty());
        });
    }

    #[test]
    fn busy_orchestrator_queues_fifo_and_one_event_each() {
        with_home(|| {
            agent("a-so2", "orchestrator", None, "working");
            let a = say("a-so2", "alice", "first", 10, Some(Some(99))).unwrap();
            let b = say("a-so2", "bob", "second", 10, Some(Some(99))).unwrap();
            assert_eq!((a.outcome, b.outcome), (SayOutcome::Queued, SayOutcome::Queued));
            assert_eq!(queue_len("a-so2"), 2);
            let ev = human_events();
            assert_eq!(ev.len(), 2);
            assert_eq!(
                (ev[0]["from"].as_str(), ev[0]["text"].as_str()),
                (Some("alice"), Some("first"))
            );
            assert_eq!(ev[0]["agent"], "a-so2");
            assert_eq!(ev[1]["id"], b.id);
            // A working turn never drains; no host means nothing to paste into either.
            assert_eq!(say_drain("a-so2").unwrap(), None);
            state_append(StateKind::Agents, "a-so2", &json!({"status": "idle"})).unwrap();
            assert_eq!(say_drain("a-so2").unwrap(), None);
            assert_eq!(queue_len("a-so2"), 2);
        });
    }

    #[test]
    fn needs_input_queues() {
        with_home(|| {
            agent("a-so3", "orchestrator", None, "needs_input");
            state_append(StateKind::Agents, "a-so3", &json!({"host": "c-none"})).unwrap();
            let r = say("a-so3", "alice", "yes", 10, Some(Some(99))).unwrap();
            assert_eq!(r.outcome, SayOutcome::Queued);
        });
    }

    #[test]
    fn answer_ask_unblocks_worker_relays_and_rejects_repeat() {
        with_home(|| {
            agent("a-ao1", "orchestrator", None, "idle");
            agent("a-aw1", "worker", Some("a-ao1"), "working");
            let ask =
                msg_send("a-aw1", "a-ao1", "ask", "which schema?", None, 10, Some(None)).unwrap();
            let id = ask.message.id;
            assert!(matches!(answer_ask("m-zzzzz", "alice", "v2"), Err(MsgError::NotFound(_))));
            let done = msg_send("a-aw1", "a-ao1", "done", "x", None, 10, Some(None)).unwrap();
            assert!(matches!(
                answer_ask(&done.message.id, "alice", "v2"),
                Err(MsgError::Invalid(_))
            ));

            let r = answer_ask(&id, "alice", "use v2").unwrap();
            assert!(r.relayed);
            assert_eq!(r.worker, "a-aw1");
            let to_worker = read_inbox_all("a-aw1").unwrap();
            assert_eq!(to_worker[0].kind, "answer");
            assert_eq!(to_worker[0].text, "[alice] use v2");
            assert_eq!(load_reported_status("a-aw1"), Some(SessionState::Idle));
            let relay =
                read_inbox_all("a-ao1").unwrap().into_iter().find(|m| m.kind == "relay").unwrap();
            assert_eq!(relay.r#ref.as_deref(), Some(id.as_str()));
            assert!(relay.text.contains("use v2"));

            let again = answer_ask(&id, "alice", "use v3").unwrap_err();
            assert!(
                matches!(&again, MsgError::Invalid(m) if m.contains("already answered")),
                "{again}"
            );
        });
    }
}
