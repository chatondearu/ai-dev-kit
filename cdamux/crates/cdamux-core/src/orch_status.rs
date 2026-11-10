//! Live orch worker status (`.orch/run/status.json`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
}

impl OrchWorkerStatus {
    /// Compact `cli·tier` for the Fleet row; `None` when neither is known.
    pub fn agent_tag(&self) -> Option<String> {
        let parts: Vec<&str> = [&self.cli, &self.tier].into_iter().flatten().map(String::as_str).collect();
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

pub fn orch_run_status_path(repo: &Path) -> PathBuf {
    repo.join(".orch/run/status.json")
}

pub fn load_orch_run_status(repo: &Path) -> Option<OrchRunStatusFile> {
    let path = orch_run_status_path(repo);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Like [`load_orch_run_status`], walking parents from `cwd` (a session may sit in a subdir).
pub fn find_orch_run_status_from_cwd(cwd: &str) -> Option<OrchRunStatusFile> {
    Path::new(cwd)
        .ancestors()
        .find_map(load_orch_run_status)
}

pub fn project_has_orch_artifacts(repo: &Path) -> bool {
    orch_run_status_path(repo).is_file() || repo.join(".orch/graph/current.json").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn loads_workers_aggregate() {
        let dir = tempfile::tempdir().unwrap();
        let path = orch_run_status_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            r#"{{"updated_at":"t","workers":[{{"id":"72","phase":"ci","message":"PR #1","updated_at":"t"}}]}}"#
        )
        .unwrap();
        let st = load_orch_run_status(dir.path()).unwrap();
        assert_eq!(st.workers.len(), 1);
        assert_eq!(st.workers[0].id, "72");
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
        let model_only = OrchWorkerStatus { model: Some("m".into()), ..Default::default() };
        assert_eq!((model_only.agent_tag(), model_only.agent_detail().as_deref()), (None, Some("m")));
    }
}
