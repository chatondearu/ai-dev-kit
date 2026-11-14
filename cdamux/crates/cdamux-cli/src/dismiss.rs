//! Fleet / navigator `d` dismiss (#336): plan + apply via shared core helpers.

use cdamux_core::{
    apply_dismiss, plan_dismiss_agent, plan_dismiss_run, tmux_has_session, ChildKind, ChildRow,
    DismissPlan, DismissReject,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DismissOutcome {
    /// Ask the operator to press `d` again (multi-entity).
    Confirm(String, String),
    Done(String),
    Rejected(&'static str),
}

fn host_alive_tmux(name: &str) -> bool {
    // Fail closed for kill/gc; for dismiss, treat probe failure as alive so we
    // never wipe a row that might still be running.
    tmux_has_session(name).unwrap_or(true)
}

fn plan_for_child(
    row: &ChildRow,
    host_alive: &dyn Fn(&str) -> bool,
) -> Result<DismissPlan, DismissReject> {
    match row.kind {
        ChildKind::Run => plan_dismiss_run(&row.id, host_alive),
        ChildKind::Worker => {
            let id = row.agent_id.as_deref().unwrap_or(row.id.as_str());
            plan_dismiss_agent(id, host_alive)
        }
        ChildKind::Subagent => Err(DismissReject::NotEligible),
    }
}

fn summarize(plan: &DismissPlan) -> String {
    format!(
        "dismissed {} agent(s), {} run(s)",
        plan.archive_agents.len(),
        plan.archive_runs.len()
    )
}

fn confirm_token(row: &ChildRow) -> Option<String> {
    match row.kind {
        ChildKind::Run => Some(format!("run:{}", row.id)),
        ChildKind::Worker => Some(format!(
            "agent:{}",
            row.agent_id.as_deref().unwrap_or(row.id.as_str())
        )),
        ChildKind::Subagent => None,
    }
}

/// Plan or apply dismiss for a Fleet child row. When `armed` matches the confirm
/// token from a prior `Confirm`, apply without asking again.
pub(crate) fn dismiss_child(row: &ChildRow, armed: Option<&str>) -> DismissOutcome {
    dismiss_child_with(row, armed, &host_alive_tmux)
}

pub(crate) fn dismiss_child_with(
    row: &ChildRow,
    armed: Option<&str>,
    host_alive: &dyn Fn(&str) -> bool,
) -> DismissOutcome {
    let Some(token) = confirm_token(row) else {
        return DismissOutcome::Rejected(DismissReject::NotEligible.message());
    };
    let plan = match plan_for_child(row, host_alive) {
        Ok(p) if !p.is_empty() => p,
        Ok(_) => return DismissOutcome::Rejected(DismissReject::NotEligible.message()),
        Err(e) => return DismissOutcome::Rejected(e.message()),
    };
    if plan.needs_confirm() && armed != Some(token.as_str()) {
        return DismissOutcome::Confirm(
            token,
            format!(
                "dismiss {} agent(s), {} run(s)? press d again",
                plan.archive_agents.len(),
                plan.archive_runs.len()
            ),
        );
    }
    match apply_dismiss(&plan) {
        Ok(done) => DismissOutcome::Done(summarize(&done)),
        Err(e) => DismissOutcome::Done(format!("dismiss failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdamux_core::{
        env_lock, state_get, state_set, PreviewSource, SessionState, StateKind,
    };
    use serde_json::json;
    use std::path::Path;

    fn with_temp_home<F: FnOnce(&Path)>(f: F) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("CDAMUX_HOME");
    }

    fn worker_row(agent_id: &str) -> ChildRow {
        ChildRow {
            id: "7".into(),
            label: "done".into(),
            kind: ChildKind::Worker,
            state: SessionState::Done,
            preview: PreviewSource::None,
            agent_id: Some(agent_id.into()),
            unread: 0,
            pending_ask: false,
            links: Default::default(),
        }
    }

    #[test]
    fn dismiss_finished_worker_archives() {
        with_temp_home(|home| {
            state_set(
                StateKind::Agents,
                "a-done",
                &json!({"id":"a-done","status":"done"}),
            )
            .unwrap();
            match dismiss_child_with(&worker_row("a-done"), None, &|_| false) {
                DismissOutcome::Done(msg) => assert!(msg.contains("agent"), "{msg}"),
                other => panic!("{other:?}"),
            }
            assert!(state_get(StateKind::Agents, "a-done").unwrap().is_none());
            assert!(home.join("archive/agents/a-done.json").is_file());
        });
    }

    #[test]
    fn dismiss_run_asks_confirm_then_applies() {
        with_temp_home(|home| {
            state_set(
                StateKind::Runs,
                "r1",
                &json!({
                    "run_id":"r1","repo":"/tmp/r","session":"dead","status":"running","nodes":[]
                }),
            )
            .unwrap();
            state_set(
                StateKind::Agents,
                "a1",
                &json!({"id":"a1","host":"dead","status":"working","run":"r1"}),
            )
            .unwrap();
            let row = ChildRow {
                id: "r1".into(),
                label: "running".into(),
                kind: ChildKind::Run,
                state: SessionState::Working,
                preview: PreviewSource::None,
                agent_id: None,
                unread: 0,
                pending_ask: false,
                links: Default::default(),
            };
            let alive = |_: &str| false;
            match dismiss_child_with(&row, None, &alive) {
                DismissOutcome::Confirm(token, msg) => {
                    assert_eq!(token, "run:r1");
                    assert!(msg.contains("press d again"), "{msg}");
                }
                other => panic!("{other:?}"),
            }
            match dismiss_child_with(&row, Some("run:r1"), &alive) {
                DismissOutcome::Done(msg) => assert!(msg.contains("run"), "{msg}"),
                other => panic!("{other:?}"),
            }
            assert!(state_get(StateKind::Runs, "r1").unwrap().is_none());
            assert!(home.join("archive/runs/r1.json").is_file());
        });
    }
}
