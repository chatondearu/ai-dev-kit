//! Finalize stuck agents/runs when a tmux host dies (PRD-307 / #307).
//! Permanent kill and `state gc --sweep-orphans` share this path.
//! Does **not** run inside raw `kill_tmux_session` (restart must keep agent state).
//! Fleet / navigator `d` dismiss (#336) reuses the same finalize + archive helpers.

use crate::orch_graph::is_live_run_status;
use crate::session_registry::{self, SessionRecord};
use crate::state::{state_get, state_list, state_set, StateKind};
use crate::state_gc::{
    agent_has_no_host, archive_agent, archive_run, is_gc_agent_status, is_gc_run_status,
    is_resumable_dormant,
};
use serde_json::Value;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// Ids touched by one finalize or sweep pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FinalizeReport {
    pub agents: Vec<String>,
    pub runs: Vec<String>,
}

fn utc_now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Match orch_graph_now (`date -u +%Y-%m-%dT%H:%M:%SZ`) without chrono.
    let days = secs / 86400;
    let tod = secs % 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Howard Hinnant civil-from-days (proleptic Gregorian), days since 1970-01-01.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn host_field(value: &Value) -> Option<&str> {
    value
        .get("host")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn session_field(value: &Value) -> Option<&str> {
    value
        .get("session")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn status_str(value: &Value) -> &str {
    value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

fn agent_already_terminal(value: &Value) -> bool {
    is_gc_agent_status(status_str(value))
}

fn finalize_agent_value(mut value: Value) -> Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert("status".into(), Value::String("error".into()));
        obj.insert("host".into(), Value::String(String::new()));
    }
    value
}

fn finalize_run_value(mut value: Value, ended: &str) -> Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert("status".into(), Value::String("error".into()));
        obj.insert("ended_at".into(), Value::String(ended.into()));
        obj.insert("updated_at".into(), Value::String(ended.into()));
        if let Some(Value::Array(nodes)) = obj.get_mut("nodes") {
            for node in nodes {
                if let Some(n) = node.as_object_mut() {
                    let st = n
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if matches!(st.as_str(), "" | "running" | "pending" | "working" | "blocked") {
                        n.insert("status".into(), Value::String("error".into()));
                        n.insert(
                            "message".into(),
                            Value::String("host session finalized".into()),
                        );
                    }
                }
            }
        }
    }
    value
}

fn run_is_live_doc(value: &Value) -> bool {
    if value.get("ended_at").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()) {
        return false;
    }
    is_live_run_status(status_str(value))
}

/// Mark agents with `host == session` and live runs with `session == name` as error.
pub fn finalize_session_host(session: &str) -> io::Result<FinalizeReport> {
    let session = session.trim();
    if session.is_empty() {
        return Ok(FinalizeReport::default());
    }
    let ended = utc_now_iso();
    let mut report = FinalizeReport::default();

    for (id, _path, value) in state_list(StateKind::Agents) {
        if host_field(&value) != Some(session) {
            continue;
        }
        // Always clear host (name reuse); set error when not already terminal.
        let next = if agent_already_terminal(&value) {
            let mut v = value;
            if let Some(obj) = v.as_object_mut() {
                obj.insert("host".into(), Value::String(String::new()));
            }
            v
        } else {
            finalize_agent_value(value)
        };
        state_set(StateKind::Agents, &id, &next)?;
        report.agents.push(id);
    }

    for (id, _path, value) in state_list(StateKind::Runs) {
        if session_field(&value) != Some(session) {
            continue;
        }
        if !run_is_live_doc(&value) {
            continue;
        }
        state_set(StateKind::Runs, &id, &finalize_run_value(value, &ended))?;
        report.runs.push(id);
    }

    report.agents.sort();
    report.runs.sort();
    Ok(report)
}

/// Whether a non-terminal agent with a dead host should be finalized by sweep.
pub fn orphan_agent_eligible(
    id: &str,
    value: &Value,
    host_alive: &dyn Fn(&str) -> bool,
    registry: &[SessionRecord],
) -> bool {
    if agent_already_terminal(value) {
        return false;
    }
    let host = match host_field(value) {
        Some(h) if !host_alive(h) => h,
        _ => return false,
    };
    !is_resumable_dormant(id, Some(host), registry)
}

/// Whether a live run whose session host is dead should be finalized by sweep.
pub fn orphan_run_eligible(value: &Value, host_alive: &dyn Fn(&str) -> bool) -> bool {
    if !run_is_live_doc(value) {
        return false;
    }
    match session_field(value) {
        Some(session) => !host_alive(session),
        // Headless live runs are not session-kill orphans.
        None => false,
    }
}

/// Finalize orphaned working/running entities whose tmux host is dead.
/// When `dry_run`, only report ids (no writes).
pub fn sweep_orphans(
    host_alive: &dyn Fn(&str) -> bool,
    dry_run: bool,
) -> io::Result<FinalizeReport> {
    let registry = session_registry::load_all();
    let ended = utc_now_iso();
    let mut report = FinalizeReport::default();

    for (id, _path, value) in state_list(StateKind::Agents) {
        if !orphan_agent_eligible(&id, &value, host_alive, &registry) {
            continue;
        }
        if !dry_run {
            state_set(StateKind::Agents, &id, &finalize_agent_value(value))?;
        }
        report.agents.push(id);
    }

    for (id, _path, value) in state_list(StateKind::Runs) {
        if !orphan_run_eligible(&value, host_alive) {
            continue;
        }
        if !dry_run {
            state_set(StateKind::Runs, &id, &finalize_run_value(value, &ended))?;
        }
        report.runs.push(id);
    }

    report.agents.sort();
    report.runs.sort();
    Ok(report)
}

/// Fleet dismiss plan: finalize orphans (same writes as sweep), then archive so
/// the row leaves live `$CDAMUX_HOME` (and the Fleet tree on refresh).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DismissPlan {
    pub finalize: FinalizeReport,
    pub archive_agents: Vec<String>,
    pub archive_runs: Vec<String>,
}

impl DismissPlan {
    /// Distinct agent/run ids touched (finalize and archive of the same id count once).
    pub fn entity_count(&self) -> usize {
        let mut agents: Vec<&str> = self
            .finalize
            .agents
            .iter()
            .chain(self.archive_agents.iter())
            .map(String::as_str)
            .collect();
        agents.sort_unstable();
        agents.dedup();
        let mut runs: Vec<&str> = self
            .finalize
            .runs
            .iter()
            .chain(self.archive_runs.iter())
            .map(String::as_str)
            .collect();
        runs.sort_unstable();
        runs.dedup();
        agents.len() + runs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entity_count() == 0
    }

    /// True when the operator must confirm (more than one entity).
    pub fn needs_confirm(&self) -> bool {
        self.entity_count() > 1
    }
}

/// Why a selected row cannot be dismissed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DismissReject {
    NotFound,
    Dormant,
    LiveWorking,
    NotEligible,
}

impl DismissReject {
    pub fn message(self) -> &'static str {
        match self {
            Self::NotFound => "dismiss: not found",
            Self::Dormant => "dismiss: resumable dormant kept (ADR-009)",
            Self::LiveWorking => "dismiss: live working row (use x to kill host)",
            Self::NotEligible => "dismiss: select a finished or orphan run/worker",
        }
    }
}

fn run_field(value: &Value) -> Option<&str> {
    value
        .get("run")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn plan_one_agent(
    id: &str,
    value: &Value,
    host_alive: &dyn Fn(&str) -> bool,
    registry: &[SessionRecord],
    plan: &mut DismissPlan,
) -> Result<(), DismissReject> {
    if is_resumable_dormant(id, host_field(value), registry) {
        return Err(DismissReject::Dormant);
    }
    if orphan_agent_eligible(id, value, host_alive, registry) {
        plan.finalize.agents.push(id.to_string());
        plan.archive_agents.push(id.to_string());
        return Ok(());
    }
    if is_gc_agent_status(status_str(value)) {
        // Finished: archive so Fleet drops the row (retention age not required).
        plan.archive_agents.push(id.to_string());
        return Ok(());
    }
    if host_field(value).is_some_and(host_alive) {
        return Err(DismissReject::LiveWorking);
    }
    // No host / dead host but not orphan-eligible (e.g. already odd state).
    if agent_has_no_host(value, host_alive) {
        plan.finalize.agents.push(id.to_string());
        plan.archive_agents.push(id.to_string());
        return Ok(());
    }
    Err(DismissReject::NotEligible)
}

fn plan_one_run(
    id: &str,
    value: &Value,
    host_alive: &dyn Fn(&str) -> bool,
    plan: &mut DismissPlan,
) -> Result<(), DismissReject> {
    if orphan_run_eligible(value, host_alive) {
        plan.finalize.runs.push(id.to_string());
        plan.archive_runs.push(id.to_string());
        return Ok(());
    }
    if is_gc_run_status(status_str(value)) {
        plan.archive_runs.push(id.to_string());
        return Ok(());
    }
    if !run_is_live_doc(value) {
        plan.archive_runs.push(id.to_string());
        return Ok(());
    }
    match session_field(value) {
        Some(session) if host_alive(session) => Err(DismissReject::LiveWorking),
        _ => {
            plan.finalize.runs.push(id.to_string());
            plan.archive_runs.push(id.to_string());
            Ok(())
        }
    }
}

/// Plan dismiss for one agent id (worker row). Does not write.
pub fn plan_dismiss_agent(
    id: &str,
    host_alive: &dyn Fn(&str) -> bool,
) -> Result<DismissPlan, DismissReject> {
    let id = id.trim();
    if id.is_empty() {
        return Err(DismissReject::NotFound);
    }
    let value = state_get(StateKind::Agents, id)
        .ok()
        .flatten()
        .ok_or(DismissReject::NotFound)?;
    let registry = session_registry::load_all();
    let mut plan = DismissPlan::default();
    plan_one_agent(id, &value, host_alive, &registry, &mut plan)?;
    plan.finalize.agents.sort();
    plan.archive_agents.sort();
    plan.archive_agents.dedup();
    Ok(plan)
}

/// Plan dismiss for one run id, cascading to agents with matching `run`.
pub fn plan_dismiss_run(
    id: &str,
    host_alive: &dyn Fn(&str) -> bool,
) -> Result<DismissPlan, DismissReject> {
    let id = id.trim();
    if id.is_empty() {
        return Err(DismissReject::NotFound);
    }
    let value = state_get(StateKind::Runs, id)
        .ok()
        .flatten()
        .ok_or(DismissReject::NotFound)?;
    let registry = session_registry::load_all();
    let mut plan = DismissPlan::default();
    plan_one_run(id, &value, host_alive, &mut plan)?;
    for (aid, _path, aval) in state_list(StateKind::Agents) {
        if run_field(&aval) != Some(id) {
            continue;
        }
        // Cascade: skip dormant and still-live workers; finalize+archive the rest.
        if is_resumable_dormant(&aid, host_field(&aval), &registry) {
            continue;
        }
        let orphan = orphan_agent_eligible(&aid, &aval, host_alive, &registry);
        let finished = is_gc_agent_status(status_str(&aval));
        let no_host = agent_has_no_host(&aval, host_alive);
        if !(orphan || finished || no_host) {
            continue;
        }
        if orphan || (!finished && no_host) {
            plan.finalize.agents.push(aid.clone());
        }
        plan.archive_agents.push(aid);
    }
    plan.finalize.agents.sort();
    plan.finalize.agents.dedup();
    plan.finalize.runs.sort();
    plan.finalize.runs.dedup();
    plan.archive_agents.sort();
    plan.archive_agents.dedup();
    plan.archive_runs.sort();
    plan.archive_runs.dedup();
    Ok(plan)
}

/// Apply a dismiss plan: finalize first (shared value writers), then archive.
pub fn apply_dismiss(plan: &DismissPlan) -> io::Result<DismissPlan> {
    let ended = utc_now_iso();
    for id in &plan.finalize.agents {
        if let Some(value) = state_get(StateKind::Agents, id)? {
            state_set(StateKind::Agents, id, &finalize_agent_value(value))?;
        }
    }
    for id in &plan.finalize.runs {
        if let Some(value) = state_get(StateKind::Runs, id)? {
            state_set(StateKind::Runs, id, &finalize_run_value(value, &ended))?;
        }
    }
    for id in &plan.archive_agents {
        archive_agent(id)?;
    }
    for id in &plan.archive_runs {
        archive_run(id)?;
    }
    Ok(plan.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::spawn::PaneLayout;
    use crate::state::{state_get, StateKind};
    use serde_json::json;
    use std::path::Path;

    fn live_run(id: &str, session: &str, repo: &str) -> Value {
        json!({
            "run_id": id,
            "repo": repo,
            "session": session,
            "status": "running",
            "nodes": [{"id": "n1", "status": "running", "message": ""}],
            "edges": []
        })
    }

    fn with_temp_home<F: FnOnce(&Path)>(f: F) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn finalize_session_host_marks_agents_and_runs() {
        with_temp_home(|home| {
            let repo = home.join("repo");
            std::fs::create_dir_all(&repo).unwrap();
            state_set(
                StateKind::Agents,
                "a-orch",
                &json!({"id":"a-orch","host":"c-proj","status":"working"}),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-other",
                &json!({"id":"a-other","host":"other","status":"working"}),
            )
            .unwrap();
            state_set(
                StateKind::Runs,
                "r-old",
                &live_run("r-old", "c-proj", &repo.to_string_lossy()),
            )
            .unwrap();
            state_set(
                StateKind::Runs,
                "r-ok",
                &json!({
                    "run_id":"r-ok","repo": repo.to_string_lossy(),
                    "session":"c-proj","status":"ok","ended_at":"t"
                }),
            )
            .unwrap();

            let report = finalize_session_host("c-proj").unwrap();
            assert_eq!(report.agents, vec!["a-orch"]);
            assert_eq!(report.runs, vec!["r-old"]);

            let orch = state_get(StateKind::Agents, "a-orch").unwrap().unwrap();
            assert_eq!(orch["status"], "error");
            assert_eq!(orch["host"], "");
            let other = state_get(StateKind::Agents, "a-other").unwrap().unwrap();
            assert_eq!(other["status"], "working");
            let old = state_get(StateKind::Runs, "r-old").unwrap().unwrap();
            assert_eq!(old["status"], "error");
            assert!(old.get("ended_at").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()));
            assert_eq!(old["nodes"][0]["status"], "error");
            let done = state_get(StateKind::Runs, "r-ok").unwrap().unwrap();
            assert_eq!(done["status"], "ok");

            // After finalize, no live run remains for that session (#307);
            // bind also requires @cdamux_run (#335).
            assert!(
                crate::orch_graph::list_live_orch_runs(&repo)
                    .iter()
                    .all(|r| r.session.as_deref() != Some("c-proj"))
            );
            assert!(crate::orch_graph::find_orch_graph_for_session(&repo, "c-proj", None).is_err());
            assert!(crate::orch_graph::find_orch_graph_for_session(
                &repo,
                "c-proj",
                Some("r-old")
            )
            .is_ok());
        });
    }

    #[test]
    fn sweep_orphans_skips_resumable_dormant_and_honours_dry_run() {
        with_temp_home(|home| {
            let repo = home.join("repo");
            std::fs::create_dir_all(&repo).unwrap();
            state_set(
                StateKind::Agents,
                "a-stuck",
                &json!({"id":"a-stuck","host":"dead-host","status":"working"}),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a-keep",
                &json!({"id":"a-keep","host":"c-dorm","status":"idle"}),
            )
            .unwrap();
            session_registry::record(&session_registry::SessionRecord {
                name: "c-dorm".into(),
                cwd: "/tmp".into(),
                agent: "claude".into(),
                agent_session_id: None,
                parent: None,
                role: "agent".into(),
                layout: PaneLayout::Single,
                created_ts: 1,
                updated_ts: 1,
                cdamux_id: Some("a-keep".into()),
            })
            .unwrap();
            state_set(
                StateKind::Runs,
                "r-dead",
                &live_run("r-dead", "dead-host", &repo.to_string_lossy()),
            )
            .unwrap();
            state_set(
                StateKind::Runs,
                "r-live",
                &live_run("r-live", "alive", &repo.to_string_lossy()),
            )
            .unwrap();

            let alive = |h: &str| h == "alive";
            let preview = sweep_orphans(&alive, true).unwrap();
            assert_eq!(preview.agents, vec!["a-stuck"]);
            assert_eq!(preview.runs, vec!["r-dead"]);
            assert_eq!(
                state_get(StateKind::Agents, "a-stuck").unwrap().unwrap()["status"],
                "working"
            );

            let applied = sweep_orphans(&alive, false).unwrap();
            assert_eq!(applied.agents, vec!["a-stuck"]);
            assert_eq!(applied.runs, vec!["r-dead"]);
            assert_eq!(
                state_get(StateKind::Agents, "a-stuck").unwrap().unwrap()["status"],
                "error"
            );
            assert_eq!(
                state_get(StateKind::Agents, "a-keep").unwrap().unwrap()["status"],
                "idle"
            );
            assert_eq!(
                state_get(StateKind::Runs, "r-live").unwrap().unwrap()["status"],
                "running"
            );
        });
    }

    #[test]
    fn dismiss_orphan_agent_finalizes_then_archives() {
        with_temp_home(|home| {
            state_set(
                StateKind::Agents,
                "a-stuck",
                &json!({"id":"a-stuck","host":"dead","status":"working","run":"r1"}),
            )
            .unwrap();
            let alive = |_: &str| false;
            let plan = plan_dismiss_agent("a-stuck", &alive).unwrap();
            assert_eq!(plan.finalize.agents, vec!["a-stuck"]);
            assert_eq!(plan.archive_agents, vec!["a-stuck"]);
            assert!(!plan.needs_confirm());
            apply_dismiss(&plan).unwrap();
            assert!(state_get(StateKind::Agents, "a-stuck").unwrap().is_none());
            assert!(home.join("archive/agents/a-stuck.json").is_file());
        });
    }

    #[test]
    fn dismiss_keeps_dormant_and_rejects_live() {
        with_temp_home(|_| {
            state_set(
                StateKind::Agents,
                "a-keep",
                &json!({"id":"a-keep","host":"c-dorm","status":"idle"}),
            )
            .unwrap();
            session_registry::record(&session_registry::SessionRecord {
                name: "c-dorm".into(),
                cwd: "/tmp".into(),
                agent: "claude".into(),
                agent_session_id: None,
                parent: None,
                role: "agent".into(),
                layout: PaneLayout::Single,
                created_ts: 1,
                updated_ts: 1,
                cdamux_id: Some("a-keep".into()),
            })
            .unwrap();
            let alive = |h: &str| h == "c-dorm" || h == "live";
            assert_eq!(
                plan_dismiss_agent("a-keep", &alive),
                Err(DismissReject::Dormant)
            );
            state_set(
                StateKind::Agents,
                "a-live",
                &json!({"id":"a-live","host":"live","status":"working"}),
            )
            .unwrap();
            assert_eq!(
                plan_dismiss_agent("a-live", &alive),
                Err(DismissReject::LiveWorking)
            );
        });
    }

    #[test]
    fn dismiss_run_cascades_workers_and_needs_confirm() {
        with_temp_home(|home| {
            let repo = home.join("repo");
            std::fs::create_dir_all(&repo).unwrap();
            state_set(
                StateKind::Runs,
                "r-dead",
                &live_run("r-dead", "gone", &repo.to_string_lossy()),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a1",
                &json!({"id":"a1","host":"gone","status":"working","run":"r-dead"}),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a2",
                &json!({"id":"a2","status":"done","run":"r-dead"}),
            )
            .unwrap();
            let alive = |_: &str| false;
            let plan = plan_dismiss_run("r-dead", &alive).unwrap();
            assert!(plan.needs_confirm(), "{plan:?}");
            assert_eq!(plan.finalize.runs, vec!["r-dead"]);
            assert!(plan.archive_agents.contains(&"a1".into()));
            assert!(plan.archive_agents.contains(&"a2".into()));
            apply_dismiss(&plan).unwrap();
            assert!(state_get(StateKind::Runs, "r-dead").unwrap().is_none());
            assert!(state_get(StateKind::Agents, "a1").unwrap().is_none());
            assert!(home.join("archive/runs/r-dead.json").is_file());
        });
    }
}
