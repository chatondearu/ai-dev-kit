//! Unified agent/run state under `CDAMUX_HOME` (ADR-011 §2).
//! Single write path for hooks / orch / workers: `set` (atomic replace) and
//! `append` (shallow JSON object merge). Format stays private.

use crate::paths::cdamux_state_dir;
use serde_json::{Map, Value};
use std::io;
use std::path::{Path, PathBuf};

/// Entity kind under `$CDAMUX_HOME/<kind>/<id>.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Agents,
    Runs,
}

impl StateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agents => "agents",
            Self::Runs => "runs",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "agents" | "agent" => Some(Self::Agents),
            "runs" | "run" => Some(Self::Runs),
            _ => None,
        }
    }
}

/// Validate an id so it cannot leave its directory (`a-xxxxx`, run ids, …).
pub fn valid_state_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && id != "."
        && id != ".."
        && id.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn entity_path(kind: StateKind, id: &str) -> io::Result<PathBuf> {
    if !valid_state_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid state id '{id}'"),
        ));
    }
    let dir = cdamux_state_dir(kind.as_str()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CDAMUX_HOME / XDG_STATE_HOME / HOME unset",
        )
    })?;
    Ok(dir.join(format!("{id}.json")))
}

/// Atomic write: tmp + rename (same pattern as the restore registry).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Replace the entity file with `value` (pretty JSON).
pub fn state_set(kind: StateKind, id: &str, value: &Value) -> io::Result<()> {
    let path = entity_path(kind, id)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    write_atomic(&path, &bytes)
}

/// Read the entity file; `None` when missing.
pub fn state_get(kind: StateKind, id: &str) -> io::Result<Option<Value>> {
    let path = entity_path(kind, id)?;
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
        Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
    }
}

/// Shallow-merge object keys from `patch` into the existing document (or create it).
/// Non-object existing values are replaced by `patch` when `patch` is an object.
pub fn state_append(kind: StateKind, id: &str, patch: &Value) -> io::Result<()> {
    let path = entity_path(kind, id)?;
    let merged = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => patch.clone(),
        Err(e) => return Err(e),
        Ok(text) => {
            let existing: Value = serde_json::from_str(&text)?;
            merge_shallow(existing, patch)
        }
    };
    let bytes = serde_json::to_vec_pretty(&merged)?;
    write_atomic(&path, &bytes)
}

fn merge_shallow(existing: Value, patch: &Value) -> Value {
    match (existing, patch) {
        (Value::Object(mut base), Value::Object(extra)) => {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
            Value::Object(base)
        }
        (_, patch) => patch.clone(),
    }
}

/// Mint `a-xxxxx` (5 lowercase alphanumeric chars). `None` without `/dev/urandom`.
pub fn generate_agent_id() -> Option<String> {
    use std::io::Read;
    const ALPH: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut b = [0u8; 5];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut b).ok()?;
    let s: String = b
        .iter()
        .map(|x| ALPH[(*x as usize) % ALPH.len()] as char)
        .collect();
    Some(format!("a-{s}"))
}

/// Mint a long-lived run id `r-xxxxx` (same entropy as [`generate_agent_id`]).
pub fn generate_run_id() -> Option<String> {
    generate_agent_id().map(|id| format!("r-{}", id.trim_start_matches("a-")))
}

/// Initial agent document written at spawn (private shape).
pub fn initial_agent_value(
    id: &str,
    client: &str,
    host: &str,
    role: &str,
    parent: Option<&str>,
) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), Value::String(id.into()));
    map.insert("client".into(), Value::String(client.into()));
    map.insert("host".into(), Value::String(host.into()));
    map.insert("role".into(), Value::String(role.into()));
    map.insert(
        "parent".into(),
        parent.map_or(Value::Null, |p| Value::String(p.into())),
    );
    map.insert("run".into(), Value::Null);
    Value::Object(map)
}

/// Path of `agents/<id>.json` when the home resolves (tests / callers).
pub fn agent_state_path(id: &str) -> Option<PathBuf> {
    entity_path(StateKind::Agents, id).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;

    fn with_temp_home<F: FnOnce(&Path)>(f: F) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn set_get_append_round_trip_and_atomic() {
        with_temp_home(|home| {
            let id = "a-test1";
            let v = serde_json::json!({"id": id, "client": "claude"});
            state_set(StateKind::Agents, id, &v).unwrap();
            let path = home.join("agents").join(format!("{id}.json"));
            assert!(path.is_file());
            assert_eq!(state_get(StateKind::Agents, id).unwrap().unwrap()["client"], "claude");

            state_append(StateKind::Agents, id, &serde_json::json!({"status": "idle"})).unwrap();
            let got = state_get(StateKind::Agents, id).unwrap().unwrap();
            assert_eq!(got["client"], "claude");
            assert_eq!(got["status"], "idle");

            state_set(
                StateKind::Runs,
                "run-1",
                &serde_json::json!({"id": "run-1", "repo": "/tmp/p"}),
            )
            .unwrap();
            assert!(home.join("runs/run-1.json").is_file());

            let leftovers = std::fs::read_dir(home.join("agents"))
                .unwrap()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
                .count();
            assert_eq!(leftovers, 0);
        });
    }

    #[test]
    fn rejects_path_escape_ids() {
        with_temp_home(|_| {
            assert!(state_set(StateKind::Agents, "../x", &serde_json::json!({})).is_err());
            assert!(state_set(StateKind::Agents, "a/b", &serde_json::json!({})).is_err());
        });
    }

    #[test]
    fn generate_agent_id_shape() {
        let id = generate_agent_id().unwrap();
        assert!(id.starts_with("a-"));
        assert_eq!(id.len(), 7);
        assert!(valid_state_id(&id));
        assert_ne!(id, generate_agent_id().unwrap());
    }

    #[test]
    fn rename_does_not_move_agent_file() {
        with_temp_home(|home| {
            use crate::session_registry::{self, SessionRecord};
            use crate::spawn::PaneLayout;

            let id = "a-stay1";
            state_set(
                StateKind::Agents,
                id,
                &initial_agent_value(id, "cursor", "c-old", "agent", None),
            )
            .unwrap();
            session_registry::record(&SessionRecord {
                name: "c-old".into(),
                cwd: "/tmp/p".into(),
                agent: "cursor".into(),
                agent_session_id: None,
                cdamux_id: Some(id.into()),
                parent: None,
                role: "agent".into(),
                layout: PaneLayout::Single,
                created_ts: 1,
                updated_ts: 1,
            })
            .unwrap();

            session_registry::rename("c-old", "c-new").unwrap();

            let after_path = home.join("agents").join(format!("{id}.json"));
            assert!(after_path.is_file());
            assert!(!home.join("agents/c-new.json").exists());
            assert!(!home.join("agents/c-old.json").exists());
            let after: Value = serde_json::from_str(&std::fs::read_to_string(&after_path).unwrap()).unwrap();
            assert_eq!(after["host"], "c-new");
            assert_eq!(after["id"], id);
        });
    }
}
