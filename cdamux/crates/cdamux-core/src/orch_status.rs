//! Live orch worker status (`.orch/run/status.json`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchWorkerStatus {
    pub id: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchRunStatusFile {
    #[serde(default)]
    pub updated_at: String,
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

pub fn project_has_orch_artifacts(repo: &Path) -> bool {
    orch_run_status_path(repo).is_file() || repo.join(".orch/graph/current.json").is_file()
}

pub fn worker_phase_icon(phase: &str) -> char {
    match phase {
        "done" => '✓',
        "blocked" => '✗',
        "pending" => '·',
        "ci" | "verify" | "agent" | "worktree" => '⚙',
        _ => '⚙',
    }
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
        assert_eq!(worker_phase_icon("ci"), '⚙');
    }
}
