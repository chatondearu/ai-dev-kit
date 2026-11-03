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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}
