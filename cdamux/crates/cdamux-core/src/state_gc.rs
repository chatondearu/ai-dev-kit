//! Retention GC for done/error agents and finished runs (PRD-239 / #259).
//! Eligible files move under `$CDAMUX_HOME/archive/` then leave live state
//! (one-pass rename). Dormant resumable agents (session registry) are kept.

use crate::paths::{cdamux_home, cdamux_state_dir};
use crate::session_registry::{self, SessionRecord};
use crate::state::{state_list, valid_state_id, StateKind};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Default retention when config omits `[state].retention_days`.
pub const DEFAULT_RETENTION_DAYS: u32 = 14;

/// Result of one `state gc` pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GcReport {
    pub archived_agents: Vec<String>,
    pub archived_runs: Vec<String>,
    pub skipped_dormant: Vec<String>,
}

/// Whether `status` is a terminal agent status eligible for GC.
pub fn is_gc_agent_status(status: &str) -> bool {
    matches!(status.trim().to_ascii_lowercase().as_str(), "done" | "error")
}

/// Whether `status` is a finished run status eligible for GC.
/// Orch finish writes `ok` / `blocked`; accept `done` / `error` too.
pub fn is_gc_run_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "done" | "error" | "ok" | "blocked"
    )
}

fn host_field(value: &Value) -> Option<&str> {
    value
        .get("host")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// True when the agent has no host field, or the host is not live.
pub fn agent_has_no_host(value: &Value, host_alive: &dyn Fn(&str) -> bool) -> bool {
    match host_field(value) {
        None => true,
        Some(h) => !host_alive(h),
    }
}

/// True when a restore-registry record still points at this agent (ADR-009).
pub fn is_resumable_dormant(
    agent_id: &str,
    host: Option<&str>,
    registry: &[SessionRecord],
) -> bool {
    registry.iter().any(|r| {
        r.cdamux_id.as_deref() == Some(agent_id) || host.is_some_and(|h| r.name == h)
    })
}

fn file_age_secs(path: &Path, now: SystemTime) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    now.duration_since(modified).ok().map(|d| d.as_secs())
}

fn older_than_retention(path: &Path, now: SystemTime, retention_days: u32) -> bool {
    let threshold = Duration::from_secs(u64::from(retention_days) * 24 * 60 * 60);
    file_age_secs(path, now).is_some_and(|age| age >= threshold.as_secs())
}

fn archive_dir(kind: &str) -> io::Result<PathBuf> {
    let home = cdamux_home().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CDAMUX_HOME / XDG_STATE_HOME / HOME unset",
        )
    })?;
    Ok(home.join("archive").join(kind))
}

/// Move `src` to `$CDAMUX_HOME/archive/<kind>/<file_name>` (creates dirs). An
/// existing archive is kept: the new one gets a `.<unix_secs>` suffix before the extension.
fn archive_then_remove(src: &Path, kind: &str) -> io::Result<PathBuf> {
    let dest_dir = archive_dir(kind)?;
    fs::create_dir_all(&dest_dir)?;
    let name = src
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing file name"))?;
    let mut dest = dest_dir.join(name);
    if dest.exists() {
        // shortcut: two archives of the same name within one second still collide.
        let stem = src.file_stem().unwrap_or(name).to_string_lossy();
        let secs = session_registry::now_secs();
        dest = dest_dir.join(match src.extension() {
            Some(ext) => format!("{stem}.{secs}.{}", ext.to_string_lossy()),
            None => format!("{stem}.{secs}"),
        });
    }
    match fs::rename(src, &dest) {
        Ok(()) => Ok(dest),
        Err(_) => {
            // Cross-device fallback: copy then delete.
            fs::copy(src, &dest)?;
            fs::remove_file(src)?;
            Ok(dest)
        }
    }
}

fn status_str(value: &Value) -> &str {
    value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

/// Eligibility for an agent entity file (does not move anything).
pub fn agent_eligible(
    id: &str,
    value: &Value,
    path: &Path,
    now: SystemTime,
    retention_days: u32,
    host_alive: &dyn Fn(&str) -> bool,
    registry: &[SessionRecord],
) -> bool {
    if !is_gc_agent_status(status_str(value)) {
        return false;
    }
    if !agent_has_no_host(value, host_alive) {
        return false;
    }
    if is_resumable_dormant(id, host_field(value), registry) {
        return false;
    }
    older_than_retention(path, now, retention_days)
}

/// Eligibility for a run entity file.
pub fn run_eligible(
    value: &Value,
    path: &Path,
    now: SystemTime,
    retention_days: u32,
    host_alive: &dyn Fn(&str) -> bool,
) -> bool {
    if !is_gc_run_status(status_str(value)) {
        return false;
    }
    // Keep a finished run while its recorded session host is still live.
    if let Some(session) = value
        .get("session")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if host_alive(session) {
            return false;
        }
    }
    older_than_retention(path, now, retention_days)
}

/// Scan `$CDAMUX_HOME` agents/runs, archive then remove eligible entities.
///
/// `retention_days` must be ≥ 1 (`0` is rejected).
pub fn run_gc(
    retention_days: u32,
    now: SystemTime,
    host_alive: &dyn Fn(&str) -> bool,
) -> io::Result<GcReport> {
    if retention_days == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "retention_days must be >= 1",
        ));
    }
    let days = retention_days;
    let registry = session_registry::load_all();
    let mut report = GcReport::default();

    let inbox_dir = cdamux_state_dir("inbox");
    for (id, path, value) in state_list(StateKind::Agents) {
        if is_resumable_dormant(&id, host_field(&value), &registry)
            && is_gc_agent_status(status_str(&value))
            && agent_has_no_host(&value, host_alive)
        {
            report.skipped_dormant.push(id.clone());
        }
        if !agent_eligible(&id, &value, &path, now, days, host_alive, &registry) {
            continue;
        }
        archive_then_remove(&path, "agents")?;
        if let Some(dir) = inbox_dir.as_deref().filter(|_| valid_state_id(&id)) {
            for ext in ["jsonl", "cursor"] {
                let file = dir.join(format!("{id}.{ext}"));
                if file.is_file() {
                    archive_then_remove(&file, "inbox")?;
                }
            }
        }
        report.archived_agents.push(id);
    }

    for (id, path, value) in state_list(StateKind::Runs) {
        if !run_eligible(&value, &path, now, days, host_alive) {
            continue;
        }
        archive_then_remove(&path, "runs")?;
        report.archived_runs.push(id);
    }

    report.archived_agents.sort();
    report.archived_runs.sort();
    report.skipped_dormant.sort();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::spawn::PaneLayout;
    use crate::state::{state_set, StateKind};
    use std::time::Duration;

    fn with_temp_home<F: FnOnce(&Path)>(f: F) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("CDAMUX_HOME");
    }

    fn set_aged_agent(id: &str, body: Value, days_ago: u64) {
        state_set(StateKind::Agents, id, &body).unwrap();
        let path = cdamux_state_dir("agents").unwrap().join(format!("{id}.json"));
        let mtime = SystemTime::now() - Duration::from_secs(days_ago * 24 * 60 * 60);
        fs::File::open(&path).unwrap().set_modified(mtime).unwrap();
    }

    fn set_aged_run(id: &str, body: Value, days_ago: u64) {
        state_set(StateKind::Runs, id, &body).unwrap();
        let path = cdamux_state_dir("runs").unwrap().join(format!("{id}.json"));
        let mtime = SystemTime::now() - Duration::from_secs(days_ago * 24 * 60 * 60);
        fs::File::open(&path).unwrap().set_modified(mtime).unwrap();
    }

    #[test]
    fn gc_archives_old_done_without_host_keeps_fresh_and_live_host() {
        with_temp_home(|home| {
            let now = SystemTime::now();
            let alive = |h: &str| h == "live-host";

            set_aged_agent(
                "a-old1",
                serde_json::json!({"id":"a-old1","status":"done","host":""}),
                15,
            );
            set_aged_agent(
                "a-err1",
                serde_json::json!({"id":"a-err1","status":"error"}),
                20,
            );
            set_aged_agent(
                "a-fresh",
                serde_json::json!({"id":"a-fresh","status":"done"}),
                2,
            );
            set_aged_agent(
                "a-live",
                serde_json::json!({"id":"a-live","status":"done","host":"live-host"}),
                30,
            );
            set_aged_agent(
                "a-idle",
                serde_json::json!({"id":"a-idle","status":"idle"}),
                30,
            );

            set_aged_run(
                "r-old",
                serde_json::json!({"run_id":"r-old","status":"ok"}),
                16,
            );
            set_aged_run(
                "r-run",
                serde_json::json!({"run_id":"r-run","status":"running"}),
                30,
            );

            let inbox = home.join("inbox");
            fs::create_dir_all(&inbox).unwrap();
            fs::write(inbox.join("a-old1.jsonl"), "{}\n").unwrap();
            fs::write(inbox.join("a-old1.cursor"), "3\n").unwrap();
            fs::write(inbox.join("a-fresh.jsonl"), "{}\n").unwrap();
            let archived = home.join("archive/agents");
            fs::create_dir_all(&archived).unwrap();
            fs::write(archived.join("a-old1.json"), "earlier").unwrap();

            let report = run_gc(14, now, &alive).unwrap();
            assert_eq!(report.archived_agents, vec!["a-err1", "a-old1"]);
            assert_eq!(report.archived_runs, vec!["r-old"]);
            assert!(home.join("agents/a-fresh.json").is_file());
            assert!(home.join("agents/a-live.json").is_file());
            assert!(home.join("agents/a-idle.json").is_file());
            assert!(home.join("runs/r-run.json").is_file());
            assert!(home.join("archive/runs/r-old.json").is_file());
            assert!(!home.join("agents/a-old1.json").exists());

            // An earlier archive of the same id is kept; the new one gets a suffix.
            assert_eq!(fs::read_to_string(archived.join("a-old1.json")).unwrap(), "earlier");
            let suffixed = fs::read_dir(&archived)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("a-old1.") && n.ends_with(".json") && n != "a-old1.json")
                .count();
            assert_eq!(suffixed, 1);

            assert!(home.join("archive/inbox/a-old1.jsonl").is_file());
            assert!(home.join("archive/inbox/a-old1.cursor").is_file());
            assert!(!inbox.join("a-old1.jsonl").exists());
            assert!(!inbox.join("a-old1.cursor").exists());
            assert!(inbox.join("a-fresh.jsonl").is_file());
        });
    }

    #[test]
    fn gc_keeps_dormant_resumable_registry_agents() {
        with_temp_home(|home| {
            let now = SystemTime::now();
            let alive = |_h: &str| false;

            set_aged_agent(
                "a-keep",
                serde_json::json!({"id":"a-keep","status":"done","host":"c-dorm"}),
                30,
            );
            session_registry::record(&SessionRecord {
                name: "c-dorm".into(),
                cwd: "/tmp/p".into(),
                agent: "claude".into(),
                agent_session_id: Some("conv-1".into()),
                cdamux_id: Some("a-keep".into()),
                parent: None,
                role: "agent".into(),
                layout: PaneLayout::Single,
                created_ts: 1,
                updated_ts: 1,
            })
            .unwrap();

            set_aged_agent(
                "a-gone",
                serde_json::json!({"id":"a-gone","status":"error"}),
                30,
            );

            let report = run_gc(14, now, &alive).unwrap();
            assert_eq!(report.archived_agents, vec!["a-gone"]);
            assert_eq!(report.skipped_dormant, vec!["a-keep"]);
            assert!(home.join("agents/a-keep.json").is_file());
            assert!(!home.join("agents/a-gone.json").exists());
        });
    }

    #[test]
    fn gc_keeps_finished_run_with_live_session() {
        with_temp_home(|home| {
            let now = SystemTime::now();
            let alive = |h: &str| h == "live-sess";

            set_aged_run(
                "r-live",
                serde_json::json!({
                    "run_id": "r-live",
                    "status": "ok",
                    "session": "live-sess"
                }),
                30,
            );
            set_aged_run(
                "r-dead",
                serde_json::json!({
                    "run_id": "r-dead",
                    "status": "ok",
                    "session": "dead-sess"
                }),
                30,
            );

            let report = run_gc(14, now, &alive).unwrap();
            assert_eq!(report.archived_runs, vec!["r-dead"]);
            assert!(home.join("runs/r-live.json").is_file());
            assert!(!home.join("runs/r-dead.json").exists());
            assert!(home.join("archive/runs/r-dead.json").is_file());
        });
    }

    #[test]
    fn run_gc_rejects_zero_retention() {
        with_temp_home(|_| {
            let err = run_gc(0, SystemTime::now(), &|_| false).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        });
    }

    #[test]
    fn config_default_retention_is_fourteen() {
        assert_eq!(DEFAULT_RETENTION_DAYS, 14);
    }
}
