//! CLI sub-agent ledger: `${XDG_STATE_HOME:-~/.local/state}/cdamux/agents/<session>.jsonl`,
//! appended by per-CLI hook adapters (schema in `docs/subagents.md`), read into
//! [`ChildRow`]s under the tmux session, plus the transcript tail used as preview.

use crate::session::SessionState;
use crate::session_links::{read_tail, ChildKind, ChildRow, PreviewSource};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Stopped sub-agents stay listed this long after their stop event.
const KEEP_STOPPED_SECS: u64 = 600;
/// Bytes read from the end of a transcript for the preview.
const TRANSCRIPT_TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Deserialize)]
struct LedgerLine {
    #[serde(default)]
    ts: u64,
    event: String,
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    transcript: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// One sub-agent folded from its start/stop events.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LedgerAgent {
    id: String,
    name: String,
    transcript: Option<PathBuf>,
    stopped_at: Option<u64>,
    failed: bool,
}

/// Ledger file of a tmux session; `None` without `XDG_STATE_HOME` or `HOME`.
pub fn subagent_ledger_path(session: &str) -> Option<PathBuf> {
    Some(cdamux_state_dir("agents")?.join(session_file_name(session, "jsonl")))
}

/// `${XDG_STATE_HOME:-$HOME/.local/state}/cdamux/<sub>`; `None` without either variable.
pub(crate) fn cdamux_state_dir(sub: &str) -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".local/state"),
    };
    Some(base.join("cdamux").join(sub))
}

/// Same rule as the hook adapters: a `/` in a session name must not leave the dir.
pub(crate) fn session_file_name(session: &str, ext: &str) -> String {
    format!("{}.{ext}", session.replace('/', "_"))
}

/// Fold ledger lines by `id`, in first-start order; malformed lines and stops
/// without a start (trimmed away) are skipped.
fn fold_ledger(text: &str) -> Vec<LedgerAgent> {
    let mut agents: Vec<LedgerAgent> = Vec::new();
    for line in text.lines() {
        let Ok(l) = serde_json::from_str::<LedgerLine>(line) else {
            continue;
        };
        let pos = agents.iter().position(|a| a.id == l.id);
        match (l.event.as_str(), pos) {
            ("start", None) => agents.push(LedgerAgent {
                id: l.id,
                name: l.name.unwrap_or_default(),
                transcript: l.transcript.map(PathBuf::from),
                stopped_at: None,
                failed: false,
            }),
            ("stop", Some(i)) => {
                let a = &mut agents[i];
                a.stopped_at = Some(l.ts);
                a.failed = l.status.as_deref() == Some("error");
                if let Some(t) = l.transcript {
                    a.transcript = Some(PathBuf::from(t));
                }
            }
            _ => {}
        }
    }
    agents
}

fn agent_rows(agents: &[LedgerAgent], now: u64) -> Vec<ChildRow> {
    agents
        .iter()
        .filter(|a| a.stopped_at.is_none_or(|t| now.saturating_sub(t) < KEEP_STOPPED_SECS))
        .map(|a| ChildRow {
            id: a.id.clone(),
            label: if a.name.is_empty() { "subagent".into() } else { a.name.clone() },
            kind: ChildKind::Subagent,
            state: match (a.stopped_at, a.failed) {
                (None, _) => SessionState::Active,
                (Some(_), true) => SessionState::Error,
                (Some(_), false) => SessionState::Done,
            },
            preview: a
                .transcript
                .clone()
                .map_or(PreviewSource::None, PreviewSource::FileTail),
        })
        .collect()
}

type LedgerCache = HashMap<PathBuf, (SystemTime, u64, Vec<LedgerAgent>)>;

/// Sub-agent rows of `session`: running ones, and stopped ones for 10 min.
/// The file is re-parsed only when its mtime or length changes.
pub fn session_subagents(session: &str) -> Vec<ChildRow> {
    static CACHE: OnceLock<Mutex<LedgerCache>> = OnceLock::new();
    let Some(path) = subagent_ledger_path(session) else {
        return Vec::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() else {
        return Vec::new();
    };
    let Some((mtime, len)) = std::fs::metadata(&path)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
    else {
        cache.remove(&path);
        return Vec::new();
    };
    if let Some((m, l, agents)) = cache.get(&path) {
        if *m == mtime && *l == len {
            return agent_rows(agents, now);
        }
    }
    let agents = fold_ledger(&std::fs::read_to_string(&path).unwrap_or_default());
    let rows = agent_rows(&agents, now);
    cache.insert(path, (mtime, len, agents));
    rows
}

/// User/assistant text of a Claude- or Cursor-style JSONL transcript (role in
/// `message.role` or at the top level); tool calls, tool results and thinking
/// blocks are skipped.
fn transcript_lines(jsonl: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in jsonl.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let role = v["message"]["role"]
            .as_str()
            .or(v["role"].as_str())
            .unwrap_or_default();
        if role != "user" && role != "assistant" {
            continue;
        }
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
        let mut text_lines = text.trim().lines();
        let Some(first) = text_lines.next() else {
            continue;
        };
        out.push(format!("{role}: {first}"));
        out.extend(text_lines.map(|l| format!("  {l}")));
    }
    out
}

type TailKey = (PathBuf, SystemTime, u64, usize);

/// Preview text for a sub-agent transcript: its last `max_lines` message lines.
/// The TUI asks every tick; the last result is reused until the file changes.
pub fn transcript_tail(path: &Path, max_lines: usize) -> String {
    static LAST: Mutex<Option<(TailKey, String)>> = Mutex::new(None);
    let key = std::fs::metadata(path)
        .ok()
        .and_then(|m| Some((path.to_path_buf(), m.modified().ok()?, m.len(), max_lines)));
    if let (Some(k), Ok(last)) = (&key, LAST.lock()) {
        if let Some((_, text)) = last.as_ref().filter(|(lk, _)| lk == k) {
            return text.clone();
        }
    }
    let Ok(tail) = read_tail(path, TRANSCRIPT_TAIL_BYTES) else {
        return "(transcript unavailable)".to_string();
    };
    let lines = transcript_lines(&tail);
    let text = if lines.is_empty() {
        "(no messages yet)".to_string()
    } else {
        lines[lines.len().saturating_sub(max_lines)..].join("\n")
    };
    if let (Some(k), Ok(mut last)) = (key, LAST.lock()) {
        *last = Some((k, text.clone()));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = r#"{"ts":100,"cli":"claude","event":"start","id":"a1","parent_id":"s","name":"Explore","transcript":"/t/agent-a1.jsonl"}
not json
{"ts":110,"cli":"claude","event":"start","id":"a2","name":"cda-review"}
{"ts":120,"cli":"claude","event":"stop","id":"a2","transcript":"/t/agent-a2.jsonl"}
{"ts":130,"cli":"claude","event":"stop","id":"gone"}
{"ts":140,"cli":"pi","event":"start","id":"a3"}
{"ts":150,"cli":"pi","event":"stop","id":"a3","status":"error"}
"#;

    #[test]
    fn ledger_folds_to_rows_and_expires_stopped() {
        let agents = fold_ledger(LEDGER);
        assert_eq!(agents.len(), 3);
        let rows = agent_rows(&agents, 200);
        let states: Vec<_> = rows.iter().map(|r| (r.id.as_str(), r.state)).collect();
        assert_eq!(
            states,
            [
                ("a1", SessionState::Active),
                ("a2", SessionState::Done),
                ("a3", SessionState::Error)
            ]
        );
        assert_eq!(rows[0].label, "Explore");
        assert_eq!(rows[2].label, "subagent");
        assert_eq!(rows[1].kind, ChildKind::Subagent);
        assert_eq!(
            rows[1].preview,
            PreviewSource::FileTail("/t/agent-a2.jsonl".into())
        );
        // 10 min after their stop, a2/a3 are gone; a1 is still running.
        let later = agent_rows(&agents, 120 + KEEP_STOPPED_SECS);
        assert_eq!(later.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["a1", "a3"]);
    }

    #[test]
    fn transcript_tail_keeps_text_and_skips_tool_noise() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("agent.jsonl");
        let jsonl = [
            r#"{"type":"user","message":{"role":"user","content":"Find the bug"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hm"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"noise"}]}}"#,
            r#"{"type":"attachment"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Found it\nin main.rs"}]}}"#,
        ]
        .join("\n");
        std::fs::write(&p, &jsonl).unwrap();
        assert_eq!(
            transcript_tail(&p, 10),
            "user: Find the bug\nassistant: Found it\n  in main.rs"
        );
        assert_eq!(transcript_tail(&p, 1), "  in main.rs");
        // A bounded read from the middle drops the partial first line.
        let last = jsonl.lines().last().unwrap().len() as u64;
        assert_eq!(
            transcript_lines(&read_tail(&p, last + 5).unwrap()),
            ["assistant: Found it", "  in main.rs"]
        );
        assert_eq!(transcript_tail(&dir.path().join("missing"), 5), "(transcript unavailable)");
    }

    #[test]
    fn transcript_lines_reads_cursor_top_level_role() {
        let jsonl = [
            r#"{"role":"user","message":{"content":[{"type":"text","text":"List files"}]}}"#,
            r#"{"role":"assistant","message":{"content":[{"type":"text","text":"Done"}]}}"#,
            r#"{"type":"turn_ended","status":"success"}"#,
        ]
        .join("\n");
        assert_eq!(transcript_lines(&jsonl), ["user: List files", "assistant: Done"]);
    }
}
