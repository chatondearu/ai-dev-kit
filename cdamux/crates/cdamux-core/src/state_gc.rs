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
    /// Inbox files (`<id>.jsonl` / `.cursor`) of agents that no longer exist (M17).
    pub archived_inboxes: Vec<String>,
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
    retention_days == 0 || older_than_retention(path, now, retention_days)
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
    retention_days == 0 || older_than_retention(path, now, retention_days)
}

/// Move one agent file (and its inbox) to `archive/`. Caller must have already
/// checked ADR-009 dormant / eligibility — this only performs the rename.
pub fn archive_agent(id: &str) -> io::Result<()> {
    let path = cdamux_state_dir("agents")
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "CDAMUX_HOME / XDG_STATE_HOME / HOME unset",
            )
        })?
        .join(format!("{id}.json"));
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("agent '{id}' not found"),
        ));
    }
    let record = crate::state::state_get(StateKind::Agents, id).ok().flatten();
    archive_then_remove(&path, "agents")?;
    crate::events::emit_lifecycle("archive", id, record.as_ref());
    if let Some(dir) = cdamux_state_dir("inbox").filter(|_| valid_state_id(id)) {
        for ext in ["jsonl", "cursor"] {
            let file = dir.join(format!("{id}.{ext}"));
            if file.is_file() {
                archive_then_remove(&file, "inbox")?;
            }
        }
    }
    Ok(())
}

/// Move one run file to `archive/`. Caller checks eligibility.
pub fn archive_run(id: &str) -> io::Result<()> {
    let path = cdamux_state_dir("runs")
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "CDAMUX_HOME / XDG_STATE_HOME / HOME unset",
            )
        })?
        .join(format!("{id}.json"));
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("run '{id}' not found"),
        ));
    }
    archive_then_remove(&path, "runs")?;
    Ok(())
}

/// Scan `$CDAMUX_HOME` agents/runs, archive then remove eligible entities.
///
/// `retention_days = 0` archives eligible `done`/`error` now (no age wait).
/// Never archives live hosts, non-terminal statuses, or ADR-009 resumable dormant.
pub fn run_gc(
    retention_days: u32,
    now: SystemTime,
    host_alive: &dyn Fn(&str) -> bool,
) -> io::Result<GcReport> {
    run_gc_ex(retention_days, now, host_alive, true)
}

/// Like [`run_gc`] but never archives `runs/` (reboot `--last-orch`: agent stubs only).
pub fn run_gc_agents_only(
    retention_days: u32,
    now: SystemTime,
    host_alive: &dyn Fn(&str) -> bool,
) -> io::Result<GcReport> {
    run_gc_ex(retention_days, now, host_alive, false)
}

fn run_gc_ex(
    retention_days: u32,
    now: SystemTime,
    host_alive: &dyn Fn(&str) -> bool,
    archive_runs: bool,
) -> io::Result<GcReport> {
    let days = retention_days;
    let registry = session_registry::load_all();
    let mut report = GcReport::default();

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
        archive_agent(&id)?;
        report.archived_agents.push(id);
    }

    if archive_runs {
        for (id, path, value) in state_list(StateKind::Runs) {
            if !run_eligible(&value, &path, now, days, host_alive) {
                continue;
            }
            archive_run(&id)?;
            report.archived_runs.push(id);
        }
    }

    if archive_runs {
        // Never "now" for inboxes: `--unrestorable` (0) still waits the default period.
        let inbox_days = if days == 0 { DEFAULT_RETENTION_DAYS } else { days };
        report.archived_inboxes = gc_orphan_inboxes(now, inbox_days)?;
    }
    report.archived_agents.sort();
    report.archived_runs.sort();
    report.skipped_dormant.sort();
    Ok(report)
}

/// Archive inbox files whose agent record is gone (archived with no inbox move, or
/// never existed: a free-form `--from`), once untouched for the retention period.
fn gc_orphan_inboxes(now: SystemTime, retention_days: u32) -> io::Result<Vec<String>> {
    let (Some(inbox), Some(agents)) = (cdamux_state_dir("inbox"), cdamux_state_dir("agents"))
    else {
        return Ok(Vec::new());
    };
    let Ok(entries) = fs::read_dir(&inbox) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        let (Some(stem), Some(ext)) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.extension().and_then(|s| s.to_str()),
        ) else {
            continue;
        };
        if !matches!(ext, "jsonl" | "cursor")
            || !valid_state_id(stem)
            || agents.join(format!("{stem}.json")).exists()
            || !older_than_retention(&path, now, retention_days)
        {
            continue;
        }
        archive_then_remove(&path, "inbox")?;
        out.push(format!("{stem}.{ext}"));
    }
    out.sort();
    Ok(out)
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
    fn gc_archives_orphan_inboxes_past_retention_only() {
        with_temp_home(|home| {
            let inbox = home.join("inbox");
            fs::create_dir_all(&inbox).unwrap();
            let old = SystemTime::now() - Duration::from_secs(15 * 24 * 60 * 60);
            for name in ["a-gone.jsonl", "a-gone.cursor", "a-kept.jsonl", "a-fresh.jsonl"] {
                fs::write(inbox.join(name), "{}\n").unwrap();
                if name != "a-fresh.jsonl" {
                    fs::File::open(inbox.join(name)).unwrap().set_modified(old).unwrap();
                }
            }
            state_set(StateKind::Agents, "a-kept", &serde_json::json!({"id": "a-kept"})).unwrap();
            let report = run_gc(14, SystemTime::now(), &|_| false).unwrap();
            assert_eq!(report.archived_inboxes, ["a-gone.cursor", "a-gone.jsonl"]);
            assert!(home.join("archive/inbox/a-gone.jsonl").is_file());
            assert!(inbox.join("a-kept.jsonl").is_file());
            assert!(inbox.join("a-fresh.jsonl").is_file());
        });
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
    fn run_gc_zero_retention_archives_fresh_unrestorable() {
        with_temp_home(|home| {
            let alive = |h: &str| h == "c-live";
            set_aged_agent(
                "a-fresh-err",
                serde_json::json!({"id":"a-fresh-err","role":"orchestrator","status":"error","host":""}),
                0,
            );
            set_aged_agent(
                "a-live-orch",
                serde_json::json!({"id":"a-live-orch","role":"orchestrator","status":"working","host":"c-live"}),
                0,
            );
            set_aged_agent(
                "a-idle",
                serde_json::json!({"id":"a-idle","status":"idle","host":""}),
                0,
            );
            session_registry::record(&SessionRecord {
                name: "c-dorm".into(),
                cwd: "/tmp/p".into(),
                agent: "cursor".into(),
                agent_session_id: Some("conv-1".into()),
                cdamux_id: Some("a-keep".into()),
                parent: None,
                role: "worker".into(),
                layout: PaneLayout::Single,
                created_ts: 1,
                updated_ts: 1,
            })
            .unwrap();
            set_aged_agent(
                "a-keep",
                serde_json::json!({"id":"a-keep","status":"done","host":"c-dorm"}),
                0,
            );

            let now = SystemTime::now();
            let report = run_gc(0, now, &alive).unwrap();
            assert_eq!(report.archived_agents, vec!["a-fresh-err"]);
            assert_eq!(report.skipped_dormant, vec!["a-keep"]);
            assert!(!home.join("agents/a-fresh-err.json").exists());
            assert!(home.join("agents/a-live-orch.json").is_file());
            assert!(home.join("agents/a-idle.json").is_file());
            assert!(home.join("agents/a-keep.json").is_file());
        });
    }

    #[test]
    fn run_gc_agents_only_keeps_finished_runs() {
        with_temp_home(|home| {
            let alive = |_h: &str| false;
            set_aged_agent(
                "a-err",
                serde_json::json!({"id":"a-err","status":"error","host":""}),
                0,
            );
            set_aged_run(
                "r-fin",
                serde_json::json!({"id":"r-fin","status":"ok","session":""}),
                0,
            );
            let now = SystemTime::now();
            let report = run_gc_agents_only(0, now, &alive).unwrap();
            assert_eq!(report.archived_agents, vec!["a-err"]);
            assert!(report.archived_runs.is_empty());
            assert!(home.join("runs/r-fin.json").is_file());
            assert!(!home.join("agents/a-err.json").exists());
        });
    }

    #[test]
    fn config_default_retention_is_fourteen() {
        assert_eq!(DEFAULT_RETENTION_DAYS, 14);
    }
}
