//! Session restore registry (ADR-009): one JSON file per session cdamux spawned,
//! `$CDAMUX_HOME/sessions/<session>.json`.
//! A restore snapshot only: tmux stays the truth for what runs, so nothing here
//! is read to decide whether a session is live. Schema in `docs/architecture.md`.

use crate::paths::cdamux_state_dir;
use crate::spawn::PaneLayout;
use crate::state::{state_append, write_atomic, StateKind};
use crate::subagent_ledger::session_file_name;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// One recorded session; the file is this struct as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// tmux session name.
    pub name: String,
    pub cwd: PathBuf,
    /// Product client id (`claude`, `cursor`, …) or `shell`.
    pub agent: String,
    /// The agent CLI's conversation id, when known (resume target).
    #[serde(default)]
    pub agent_session_id: Option<String>,
    /// Stable cdamux agent id (`a-xxxxx`); tmux rename never moves its state file.
    #[serde(default)]
    pub cdamux_id: Option<String>,
    /// Fork source session (`@cdamux_parent`).
    #[serde(default)]
    pub parent: Option<String>,
    /// `@cdamux_role`: `agent`, `shell`, `orchestrator`, or `worker`.
    pub role: String,
    #[serde(default)]
    pub layout: PaneLayout,
    /// Unix seconds.
    pub created_ts: u64,
    pub updated_ts: u64,
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Directory holding the records; its mtime moves on every write, rename and removal
/// (writes land by rename), which the Fleet refresh uses to skip re-reading it.
pub fn registry_dir() -> Option<PathBuf> {
    cdamux_state_dir("sessions")
}

fn record_path(name: &str) -> io::Result<PathBuf> {
    let dir = registry_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CDAMUX_HOME / XDG_STATE_HOME / HOME unset",
        )
    })?;
    Ok(dir.join(session_file_name(name, "json")))
}

/// Write (or overwrite) the record of `rec.name`; tmp file + rename, so a reader
/// never sees a half-written file.
pub fn record(rec: &SessionRecord) -> io::Result<()> {
    write_atomic(&record_path(&rec.name)?, &serde_json::to_vec_pretty(rec)?)
}

/// The record of `name`; `None` when missing or unreadable.
pub fn load(name: &str) -> Option<SessionRecord> {
    let text = std::fs::read_to_string(record_path(name).ok()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Every readable record, sorted by name (malformed files are skipped).
pub fn load_all() -> Vec<SessionRecord> {
    let Some(dir) = registry_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<SessionRecord> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Forget `name`; a missing record is not an error.
pub fn remove(name: &str) -> io::Result<()> {
    match std::fs::remove_file(record_path(name)?) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

/// Move the record of `old` to `new` (tmux rename); no record → nothing to do.
/// Updates `host` on `agents/<cdamux_id>.json` when present; never moves that file.
pub fn rename(old: &str, new: &str) -> io::Result<()> {
    let Some(mut rec) = load(old) else {
        return Ok(());
    };
    rec.name = new.to_string();
    rec.updated_ts = now_secs();
    record(&rec)?;
    remove(old)?;
    if let Some(id) = rec.cdamux_id.as_deref() {
        let _ = state_append(StateKind::Agents, id, &json!({ "host": new }));
    }
    Ok(())
}

/// Store the agent conversation id of `name`; `NotFound` when it has no record.
pub fn set_agent_session_id(name: &str, id: &str) -> io::Result<()> {
    let mut rec = load(name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no record for session '{name}'")))?;
    rec.agent_session_id = Some(id.to_string());
    rec.updated_ts = now_secs();
    record(&rec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str) -> SessionRecord {
        SessionRecord {
            name: name.into(),
            cwd: "/tmp/proj".into(),
            agent: "claude".into(),
            agent_session_id: None,
            cdamux_id: Some("a-abc12".into()),
            parent: Some("c-main".into()),
            role: "agent".into(),
            layout: PaneLayout::Horizontal,
            created_ts: 1,
            updated_ts: 1,
        }
    }

    // Sets CDAMUX_HOME; serialised with other env-sensitive tests via paths::env_lock.
    #[test]
    fn registry_round_trip_rename_remove_and_slash_names() {
        let _g = crate::paths::env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());

        record(&rec("c-a")).unwrap();
        assert_eq!(load("c-a"), Some(rec("c-a")));

        // `/` stays inside the sessions dir.
        record(&rec("feat/x")).unwrap();
        assert!(dir.path().join("sessions/feat_x.json").is_file());
        assert_eq!(load("feat/x").unwrap().name, "feat/x");

        set_agent_session_id("c-a", "abc").unwrap();
        assert_eq!(load("c-a").unwrap().agent_session_id.as_deref(), Some("abc"));
        assert!(set_agent_session_id("missing", "x").is_err());

        rename("c-a", "c-b").unwrap();
        assert_eq!(load("c-a"), None);
        let b = load("c-b").unwrap();
        assert_eq!(
            (b.name.as_str(), b.agent_session_id.as_deref(), b.cdamux_id.as_deref()),
            ("c-b", Some("abc"), Some("a-abc12"))
        );
        rename("missing", "other").unwrap();

        std::fs::write(dir.path().join("sessions/bad.json"), "{").unwrap();
        let names: Vec<String> = load_all().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["c-b", "feat/x"]);

        remove("c-b").unwrap();
        remove("c-b").unwrap();
        assert_eq!(load("c-b"), None);
        // No tmp file left behind by the atomic writes.
        let leftovers = std::fs::read_dir(dir.path().join("sessions"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
        std::env::remove_var("CDAMUX_HOME");
    }
}
