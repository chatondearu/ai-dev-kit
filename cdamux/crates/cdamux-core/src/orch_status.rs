//! Orch worker status from `$CDAMUX_HOME/agents/` (ADR-011 §5 hard cut #252).
//! Fleet / `/v1/orch-status` map worker Agents onto the historical
//! [`OrchRunStatusFile`] shape (phase, tmux_session, …).

use crate::orch_graph::{list_live_orch_runs, list_orch_graph_runs};
use crate::paths::{cdamux_home, cdamux_state_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// Same fingerprint idea as run listing: dir mtime alone misses in-place rewrites.
#[derive(Clone, Copy, PartialEq, Eq)]
struct DirStamp {
    dir_mtime: Option<SystemTime>,
    json_count: usize,
    newest_file: Option<SystemTime>,
}

fn dir_stamp(dir: &Path) -> DirStamp {
    let dir_mtime = fs::metadata(dir).ok().and_then(|m| m.modified().ok());
    let mut json_count = 0;
    let mut newest_file = None;
    if let Ok(rd) = fs::read_dir(dir) {
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            json_count += 1;
            if let Some(m) = ent.metadata().ok().and_then(|m| m.modified().ok()) {
                if newest_file.is_none_or(|n| m > n) {
                    newest_file = Some(m);
                }
            }
        }
    }
    DirStamp {
        dir_mtime,
        json_count,
        newest_file,
    }
}

/// One phase change: `started_at` is epoch seconds (written by `lib/progress.sh`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhaseMark {
    pub phase: String,
    pub started_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchWorkerStatus {
    pub id: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub updated_at: String,
    /// Phase history, oldest first; absent in files written before it existed.
    #[serde(default)]
    pub phases: Vec<PhaseMark>,
    /// `ORCH_TMUX=1`: tmux session running the worker's agent (may be gone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
    /// `ORCH_TMUX=1`: agent output log of the worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<String>,
    /// Agent the worker runs (set by `orch_progress_agent`; absent in older files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// `$CDAMUX_HOME/agents/<id>.json` stem (Fleet inbox badges / answer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Parent orchestrator agent id when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl OrchWorkerStatus {
    /// Compact `cli·tier` for the Fleet row; `None` when neither is known.
    pub fn agent_tag(&self) -> Option<String> {
        let parts: Vec<&str> = [&self.cli, &self.tier]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        (!parts.is_empty()).then(|| parts.join("·"))
    }

    /// Sidecar line: `cli·tier model effort`; `None` when nothing is known.
    pub fn agent_detail(&self) -> Option<String> {
        let mut parts: Vec<String> = self.agent_tag().into_iter().collect();
        parts.extend([&self.model, &self.effort].into_iter().flatten().cloned());
        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchRunStatusFile {
    #[serde(default)]
    pub updated_at: String,
    /// Orch run the workers belong to (same id as the graph file and `@cdamux_run`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default)]
    pub workers: Vec<OrchWorkerStatus>,
}

/// `$CDAMUX_HOME/agents` when the home resolves.
pub fn orch_agents_dir() -> Option<PathBuf> {
    cdamux_state_dir("agents")
}

#[derive(Debug, Deserialize)]
struct AgentDoc {
    #[serde(default)]
    role: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    phases: Vec<PhaseMark>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    tmux_session: Option<String>,
    #[serde(default)]
    log: Option<String>,
    #[serde(default)]
    run: Option<String>,
    #[serde(default)]
    client: Option<String>,
    #[serde(default)]
    cli: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    parent: Option<String>,
}

fn phase_from_agent(a: &AgentDoc) -> String {
    if let Some(p) = a.phase.as_deref().filter(|s| !s.is_empty()) {
        return p.to_string();
    }
    match a.status.as_str() {
        "done" => "done".into(),
        "blocked" | "error" => "blocked".into(),
        "idle" | "dormant" => "pending".into(),
        _ => "agent".into(),
    }
}

fn worker_from_agent(agent_id: String, a: AgentDoc) -> Option<OrchWorkerStatus> {
    if a.role != "worker" || a.name.is_empty() {
        return None;
    }
    let phase = phase_from_agent(&a);
    let tmux = a
        .tmux_session
        .filter(|s| !s.is_empty())
        .or_else(|| a.host.filter(|s| !s.is_empty()));
    Some(OrchWorkerStatus {
        id: a.name,
        phase,
        message: a.message.unwrap_or_default(),
        updated_at: a.updated_at.unwrap_or_default(),
        phases: a.phases,
        tmux_session: tmux,
        log: a.log.filter(|s| !s.is_empty()),
        cli: a.cli.or(a.client).filter(|s| !s.is_empty() && s != "unknown"),
        model: a.model.filter(|s| !s.is_empty()),
        effort: a.effort.filter(|s| !s.is_empty()),
        tier: a.tier.filter(|s| !s.is_empty()),
        agent_id: Some(agent_id),
        parent: a.parent.filter(|s| !s.is_empty()),
    })
}

/// Parsed worker row keyed by its run id (mtime-gated agents dir index).
#[derive(Clone)]
struct IndexedAgent {
    run: String,
    updated_at: String,
    worker: OrchWorkerStatus,
}

struct AgentsIndex {
    home: PathBuf,
    stamp: DirStamp,
    agents: Vec<IndexedAgent>,
}

fn agents_index_lock() -> std::sync::MutexGuard<'static, Option<AgentsIndex>> {
    static CACHE: OnceLock<Mutex<Option<AgentsIndex>>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn rebuild_agents_index(home: PathBuf, stamp: DirStamp) -> AgentsIndex {
    let dir = home.join("agents");
    let mut agents = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(doc) = serde_json::from_str::<AgentDoc>(&raw) else {
                continue;
            };
            let Some(run) = doc
                .run
                .as_ref()
                .map(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
            else {
                continue;
            };
            let agent_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if agent_id.is_empty() {
                continue;
            }
            let updated_at = doc.updated_at.clone().unwrap_or_default();
            if let Some(worker) = worker_from_agent(agent_id, doc) {
                agents.push(IndexedAgent {
                    run,
                    updated_at,
                    worker,
                });
            }
        }
    }
    AgentsIndex { home, stamp, agents }
}

fn agents_for_home() -> Vec<IndexedAgent> {
    let Some(home) = cdamux_home() else {
        return Vec::new();
    };
    let stamp = dir_stamp(&home.join("agents"));
    let mut guard = agents_index_lock();
    let needs_rebuild = match guard.as_ref() {
        Some(idx) => idx.home != home || idx.stamp != stamp,
        None => true,
    };
    if needs_rebuild {
        *guard = Some(rebuild_agents_index(home, stamp));
    }
    guard
        .as_ref()
        .map(|idx| idx.agents.clone())
        .unwrap_or_default()
}

/// Load workers for `repo` from `$CDAMUX_HOME/agents`, scoped to that repo's runs.
pub fn load_orch_run_status(repo: &Path) -> Option<OrchRunStatusFile> {
    let live = list_live_orch_runs(repo);
    let recent = list_orch_graph_runs(repo, 20);
    if live.is_empty() && recent.is_empty() {
        return None;
    }
    let live_ids: std::collections::HashSet<String> =
        live.iter().map(|r| r.run_id.clone()).collect();
    // Workers of live runs, else of the newest finished run.
    let scope: std::collections::HashSet<String> = if !live_ids.is_empty() {
        live_ids
    } else {
        recent.iter().take(1).map(|r| r.run_id.clone()).collect()
    };
    let preferred_run = live
        .first()
        .or(recent.first())
        .map(|r| r.run_id.clone());

    let mut workers = Vec::new();
    let mut updated_at = String::new();
    // Agents dir is parsed once per directory mtime (shared across repos on refresh).
    for a in agents_for_home() {
        if !scope.contains(&a.run) {
            continue;
        }
        if a.updated_at.as_str() > updated_at.as_str() {
            updated_at = a.updated_at;
        }
        workers.push(a.worker);
    }
    workers.sort_by(|a, b| a.id.cmp(&b.id));
    Some(OrchRunStatusFile {
        updated_at,
        run_id: preferred_run,
        workers,
    })
}

/// Like [`load_orch_run_status`], walking parents from `cwd`.
pub fn find_orch_run_status_from_cwd(cwd: &str) -> Option<OrchRunStatusFile> {
    Path::new(cwd)
        .ancestors()
        .find_map(load_orch_run_status)
}

pub fn project_has_orch_artifacts(repo: &Path) -> bool {
    if cdamux_home().is_none() {
        return false;
    }
    !list_live_orch_runs(repo).is_empty() || !list_orch_graph_runs(repo, 1).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::state::{state_set, StateKind};
    use std::io::Write;

    #[test]
    fn loads_workers_from_agents_under_cdamux_home() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let home = dir.path().join("cdamux");
        std::env::set_var("CDAMUX_HOME", &home);

        let repo_s = repo.to_string_lossy().to_string();
        state_set(
            StateKind::Runs,
            "r1",
            &serde_json::json!({
                "run_id": "r1", "id": "r1", "repo": repo_s,
                "status": "running", "nodes": [], "edges": []
            }),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-w72",
            &serde_json::json!({
                "id": "a-w72", "role": "worker", "name": "72",
                "status": "working", "phase": "ci", "message": "PR #1",
                "updated_at": "t", "run": "r1",
                "cli": "claude", "tier": "medium"
            }),
        )
        .unwrap();

        let st = load_orch_run_status(&repo).unwrap();
        assert_eq!(st.workers.len(), 1);
        assert_eq!(st.workers[0].id, "72");
        assert_eq!(st.workers[0].phase, "ci");
        assert_eq!(st.run_id.as_deref(), Some("r1"));

        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn agent_fields_optional_and_formatted() {
        let old: OrchWorkerStatus = serde_json::from_str(r#"{"id":"1","phase":"agent"}"#).unwrap();
        assert_eq!((old.agent_tag(), old.agent_detail()), (None, None));
        assert!(!serde_json::to_string(&old).unwrap().contains("tier"));

        let new: OrchWorkerStatus = serde_json::from_str(
            r#"{"id":"1","cli":"claude","model":"opus","effort":"high","tier":"hard"}"#,
        )
        .unwrap();
        assert_eq!(new.agent_tag().as_deref(), Some("claude·hard"));
        assert_eq!(new.agent_detail().as_deref(), Some("claude·hard opus high"));
        let model_only = OrchWorkerStatus {
            model: Some("m".into()),
            ..Default::default()
        };
        assert_eq!(
            (model_only.agent_tag(), model_only.agent_detail().as_deref()),
            (None, Some("m"))
        );
    }

    #[test]
    fn legacy_status_json_ignored_without_cdamux_agents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".orch/run/status.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{"updated_at":"t","workers":[{{"id":"72","phase":"ci"}}]}}"#
        )
        .unwrap();
        // Without CDAMUX_HOME agents/runs, the legacy file is ignored.
        assert!(load_orch_run_status(dir.path()).is_none());
    }
}
