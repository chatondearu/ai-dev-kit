//! Human → orchestrator: `cdamux say` pastes a labelled prompt into an idle orchestrator
//! or queues it (`$CDAMUX_HOME/say/<id>.jsonl`, FIFO) until a turn ends; `msg answer`
//! answers a worker's ask on the human's behalf. Spec: `docs/say.md`.

use crate::agent_status::load_reported_status;
use crate::events::emit;
use crate::hooklog::hook_log;
use crate::msg::{
    agent_role_and_host, ask_answered, check_text, find_message, generate_msg_id, inbox_path,
    msg_send, read_inbox_all, resolve_agent_id, should_paste, MsgError,
};
use crate::paths::cdamux_state_dir;
use crate::send::send_to_session;
use crate::session::SessionState;
use crate::state::{state_append, with_entity_lock, write_atomic, StateKind};
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
        let mut f = crate::paths::private_file_options().append(true).open(&path)?;
        f.write_all(line.as_bytes())
    })?;
    Ok(())
}

/// Paste (`paste`: pastes into the orchestrator pane, `true` on success) the oldest queued
/// prompt when `ready()` (checked under the queue lock) and drop it from the queue; returns
/// its id. The lock spans the check, the paste and the drop, and the drop only follows a
/// successful paste: a hook killed mid-way repeats a prompt rather than losing it, and two
/// callers never paste twice. A paste emits one `delivered` item (the `say` id). A pasted
/// prompt marks a hooked orchestrator at rest (`idle`, sticky `done` / `error`) `working`
/// under the same lock, so a concurrent `say` or drain queues until the turn's `Stop` hook
/// reports `idle`.
fn deliver_next(
    orch: &str,
    paste: impl FnOnce(&str) -> bool,
    ready: impl FnOnce() -> bool,
) -> Result<Option<String>, MsgError> {
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
        let log = |decision| hook_log("say", orch, decision, json!({ "id": q.id }));
        if !ready() {
            log("not_ready");
            return Ok(None);
        }
        if !paste(&format!("[{}] {}", q.from, q.text)) {
            log("paste_failed");
            return Ok(None);
        }
        let rest: String = lines.map(|l| format!("{l}\n")).collect();
        write_atomic(&path, rest.as_bytes())?;
        emit(json!({ "type": "delivered", "ts": now_ts(), "id": q.id, "agent": orch }));
        log("pasted");
        if matches!(
            load_reported_status(orch),
            Some(SessionState::Idle | SessionState::Done | SessionState::Error)
        ) {
            // Best effort: the prompt is already delivered; an error must not invite a retry.
            let _ = state_append(StateKind::Agents, orch, &json!({ "status": "working" }));
        }
        Ok(Some(q.id))
    })?)
}

/// Longest `--from` label, in bytes.
pub const MAX_LABEL_BYTES: usize = 64;

/// One-line `--from` label of at most [`MAX_LABEL_BYTES`], without control characters or
/// brackets: a `]` would let the label forge another `[label]` prefix.
fn check_label(s: &str) -> Result<(), MsgError> {
    if s.trim().is_empty()
        || s.len() > MAX_LABEL_BYTES
        || s.chars().any(|c| c.is_control() || c == '[' || c == ']')
    {
        return Err(MsgError::Invalid(format!(
            "--from must be a non-empty one-line label of at most {MAX_LABEL_BYTES} bytes, \
             without control characters or brackets"
        )));
    }
    Ok(())
}

/// Deliver `text` to an orchestrator as a human prompt `[from] text`: pasted when it is
/// at rest (reported `idle`, sticky `done` / `error`, or no status, and a quiet pane), else queued for its next turn
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
    check_label(from)?;
    check_text(text)?;
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
    let at_rest = should_paste(status, Some(u32::MAX), quiet_secs);
    if let (true, Some(s)) = (at_rest, idle) {
        if s < quiet_secs {
            std::thread::sleep(Duration::from_secs(u64::from(quiet_secs - s)));
            (status, idle) = snapshot();
        }
    }
    // Fresh snapshot under the queue lock: a concurrent `say` may have just pasted.
    let ready = || {
        let (status, idle) = snapshot();
        should_paste(status, idle, quiet_secs)
    };
    let delivered = match host.as_deref().filter(|_| should_paste(status, idle, quiet_secs)) {
        Some(h) => deliver_next(&to, |t| paste_into(h, t), ready)?.as_deref() == Some(id.as_str()),
        None => false,
    };
    let outcome = if delivered { SayOutcome::Delivered } else { SayOutcome::Queued };
    hook_log(
        "say",
        &to,
        outcome.as_str(),
        json!({ "id": id, "status": status, "idle": idle, "host": host.is_some() }),
    );
    Ok(SayResult { id, target: to, outcome })
}

fn paste_into(host: &str, text: &str) -> bool {
    send_to_session(host, text, true).is_ok()
}

/// Turn-end hook (`cdamux msg drain`): paste the oldest queued prompt of an orchestrator
/// that reports `idle`. One prompt per turn end: the paste starts the next turn.
/// Returns the delivered prompt id.
pub fn say_drain(agent: &str) -> Result<Option<String>, MsgError> {
    let (role, host) = agent_role_and_host(agent)?;
    if role != "orchestrator" {
        return Ok(None);
    }
    let idle = || load_reported_status(agent) == Some(SessionState::Idle);
    host.map_or(Ok(None), |h| deliver_next(agent, |t| paste_into(&h, t), idle))
}

/// Answer one `ask` message on the human's behalf: the worker gets an `answer` from the
/// asked agent with `ref` = the ask id (so only that ask counts as answered), and the asked
/// agent's inbox gets a `relay` copy. Unknown id, a message that is not an ask, or an ask
/// already answered: `NotFound` / `Invalid`.
pub fn answer_ask(ask_id: &str, from: &str, text: &str) -> Result<AnswerResult, MsgError> {
    check_label(from)?;
    check_text(text)?;
    let from = from.trim();
    let ask = find_message(ask_id)?.ok_or_else(|| MsgError::NotFound(format!("ask {ask_id}")))?;
    if ask.kind != "ask" {
        return Err(MsgError::Invalid(format!("message {ask_id} is a '{}', not an ask", ask.kind)));
    }
    // Check and answer under one lock: two concurrent answers to one ask cannot both pass.
    let out = with_entity_lock(&inbox_path(&ask.from)?, || {
        let answered = read_inbox_all(&ask.from).map(|msgs| ask_answered(&ask, &msgs));
        Ok(match answered {
            Ok(true) => Err(MsgError::Invalid(format!("ask {ask_id} is already answered"))),
            Ok(false) => msg_send(
                &ask.to,
                &ask.from,
                "answer",
                &format!("[{from}] {text}"),
                Some(ask_id),
                None,
            ),
            Err(e) => Err(e),
        })
    })??;
    let relay = format!("[{from}] answered {}'s ask {ask_id}: {text}", ask.from);
    let relayed = msg_send(
        "human", &ask.to, "relay", &relay, Some(ask_id), None,
    )
    .is_ok();
    hook_log(
        "answer",
        &ask.to,
        if relayed { "answered" } else { "relay_failed" },
        json!({ "id": out.message.id, "ask": ask_id, "worker": ask.from }),
    );
    Ok(AnswerResult { id: out.message.id, ask: ask_id.into(), worker: ask.from, relayed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::{has_pending_ask, MAX_MSG_BYTES};
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
            assert!(matches!(say("a-so1", "alice", "a\r\nb", 10, None), Err(MsgError::Invalid(_))));
            let forged = say("a-so1", "x] [human via neomind/console", "hi", 10, None);
            assert!(matches!(forged, Err(MsgError::Invalid(_))));
            let long = "l".repeat(MAX_LABEL_BYTES + 1);
            assert!(matches!(say("a-so1", &long, "hi", 10, None), Err(MsgError::Invalid(_))));
            let big = "x".repeat(MAX_MSG_BYTES + 1);
            assert_eq!(say("a-so1", "alice", &big, 10, None), Err(MsgError::TooLarge));
            assert!(human_events().is_empty());
            // Tabs and newlines are text, like `cdamux send`.
            let r = say("a-so1", "alice", "a\tb\nc", 10, Some(None)).unwrap();
            assert_eq!(r.outcome, SayOutcome::Queued);
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
    fn paste_emits_one_delivered_item_with_the_say_id() {
        with_home(|| {
            agent("a-so5", "orchestrator", None, "working");
            let a = say("a-so5", "alice", "first", 10, Some(Some(99))).unwrap();
            let b = say("a-so5", "bob", "second", 10, Some(Some(99))).unwrap();
            let delivered = || -> Vec<Value> {
                crate::events::read_events(0)
                    .unwrap()
                    .into_iter()
                    .filter(|v| v["type"] == "delivered")
                    .collect()
            };
            assert!(delivered().is_empty(), "queued is not delivered");

            // Not ready, or a failed paste: nothing popped, no item.
            assert_eq!(deliver_next("a-so5", |_| true, || false).unwrap(), None);
            assert_eq!(deliver_next("a-so5", |_| false, || true).unwrap(), None);
            assert!(delivered().is_empty());
            assert_eq!(queue_len("a-so5"), 2);

            let mut pasted = Vec::new();
            let got = deliver_next("a-so5", |t| { pasted.push(t.to_string()); true }, || true);
            assert_eq!(got.unwrap().as_deref(), Some(a.id.as_str()));
            assert_eq!(pasted, ["[alice] first"]);
            assert_eq!(queue_len("a-so5"), 1);
            let d = delivered();
            assert_eq!(d.len(), 1);
            assert_eq!((d[0]["id"].as_str(), d[0]["agent"].as_str()), (Some(a.id.as_str()), Some("a-so5")));
            assert!(d[0].get("text").is_none(), "the human item already carries the text");

            deliver_next("a-so5", |_| true, || true).unwrap();
            let ids: Vec<Value> = delivered().into_iter().map(|v| v["id"].clone()).collect();
            assert_eq!(ids, [json!(a.id), json!(b.id)]);

            // The hook log traces each decision by id, never the text.
            let log = std::fs::read_to_string(
                crate::paths::cdamux_state_dir("logs").unwrap().join("hooks.log"),
            )
            .unwrap();
            let decisions: Vec<String> = log
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .map(|v| format!("{}:{}", v["decision"].as_str().unwrap(), v["id"].as_str().unwrap()))
                .collect();
            let (a, b) = (&a.id, &b.id);
            let want = [
                format!("queued:{a}"),
                format!("queued:{b}"),
                format!("not_ready:{a}"),
                format!("paste_failed:{a}"),
                format!("pasted:{a}"),
                format!("pasted:{b}"),
            ];
            assert_eq!(decisions, want);
            assert!(!log.contains("first") && !log.contains("second"));
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
                msg_send("a-aw1", "a-ao1", "ask", "which schema?", None, Some(None)).unwrap();
            let id = ask.message.id;
            assert!(matches!(answer_ask("m-zzzzz", "alice", "v2"), Err(MsgError::NotFound(_))));
            // Not `done`: the worker's own done would replace `blocked` (msg.rs).
            let report = msg_send("a-aw1", "a-ao1", "report", "x", None, Some(None)).unwrap();
            assert!(matches!(
                answer_ask(&report.message.id, "alice", "v2"),
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

            assert_eq!(to_worker[0].r#ref.as_deref(), Some(id.as_str()));

            let again = answer_ask(&id, "alice", "use v3").unwrap_err();
            assert!(
                matches!(&again, MsgError::Invalid(m) if m.contains("already answered")),
                "{again}"
            );
        });
    }

    #[test]
    fn answer_targets_one_ask() {
        with_home(|| {
            agent("a-ao2", "orchestrator", None, "idle");
            agent("a-ao3", "orchestrator", None, "idle");
            agent("a-aw2", "worker", Some("a-ao2"), "working");
            let ask = |to: &str, t: &str| {
                msg_send("a-aw2", to, "ask", t, None, Some(None)).unwrap().message.id
            };
            // Same second: a timestamp match would mark both answered.
            let (a, b) = (ask("a-ao2", "schema?"), ask("a-ao2", "branch?"));
            answer_ask(&a, "alice", "v2").unwrap();
            assert!(has_pending_ask("a-aw2", "a-ao2"), "b is still open");
            answer_ask(&b, "alice", "main").unwrap();
            assert!(!has_pending_ask("a-aw2", "a-ao2"));
            assert!(matches!(answer_ask(&b, "bob", "dev"), Err(MsgError::Invalid(_))));

            // An ask sent past the parent is answered by the asked agent; repeats still fail.
            let c = ask("a-ao3", "deploy?");
            let r = answer_ask(&c, "alice", "no").unwrap();
            let ans = read_inbox_all("a-aw2").unwrap().into_iter().find(|m| m.id == r.id).unwrap();
            assert_eq!(ans.from, "a-ao3");
            assert!(matches!(answer_ask(&c, "alice", "yes"), Err(MsgError::Invalid(_))));
        });
    }

    #[test]
    fn answer_with_path_ref_stays_worker_level() {
        with_home(|| {
            agent("a-ao4", "orchestrator", None, "idle");
            agent("a-aw4", "worker", Some("a-ao4"), "working");
            let ask = |t: &str| {
                msg_send("a-aw4", "a-ao4", "ask", t, None, Some(None)).unwrap().message.id
            };
            let (a, b) = (ask("schema?"), ask("branch?"));
            // `msg send --kind answer --ref <path>`: not an ask id, answers both asks.
            msg_send("a-ao4", "a-aw4", "answer", "see doc", Some("notes/a.md"), Some(None))
                .unwrap();
            assert!(!has_pending_ask("a-aw4", "a-ao4"));
            for id in [&a, &b] {
                assert!(matches!(answer_ask(id, "alice", "x"), Err(MsgError::Invalid(_))));
            }
            // An id ref answers that ask only (own worker: the path-ref answer above has the
            // same second, so it would count for any ask sent now).
            agent("a-aw5", "worker", Some("a-ao4"), "working");
            msg_send("a-aw5", "a-ao4", "ask", "deploy?", None, Some(None)).unwrap();
            msg_send("a-ao4", "a-aw5", "answer", "no", Some("m-zzzzz"), Some(None)).unwrap();
            assert!(has_pending_ask("a-aw5", "a-ao4"));
        });
    }
}
