//! Restore recorded sessions (ADR-009): recreate a dormant session from its registry
//! record and resume the agent on the same conversation when the agent supports it.

use crate::agent_status::load_reported_status;
use crate::client_name::normalize_client_name;
use crate::send::tmux_has_session;
use crate::session::SessionState;
use crate::session_manage::{is_home_session, is_nav_session};
use crate::session_registry::{self, SessionRecord};
use crate::spawn::{
    agent_entry, default_shell_command, resolve_agent_command, respawn_recorded,
    with_claude_session_id, SpawnError, SpawnOutcome,
};
use crate::state::{state_get, StateKind};
use crate::tmux::SessionPaneInfo;
use crate::AgentsConfig;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RestoreError {
    #[error("{0}")]
    Spawn(#[from] SpawnError),
    #[error("tmux: cannot check whether the session is live")]
    LiveCheck,
}

/// What `restore_one` did with a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreOutcome {
    /// Recreated on the recorded conversation (or a shell reopened).
    Restored(SpawnOutcome),
    /// Recreated with a fresh conversation; `reason` says why it could not resume.
    Fresh { reason: &'static str, spawn: SpawnOutcome },
    /// A tmux session with that name already runs: nothing done.
    SkippedLive,
}

impl RestoreOutcome {
    /// One status word for `cdamux restore` lines and Fleet status messages.
    pub fn label(&self) -> String {
        match self {
            Self::Restored(o) => format!("restored{}", o.note()),
            Self::Fresh { reason, spawn } => format!("fresh ({reason}){}", spawn.note()),
            Self::SkippedLive => "skipped (live)".to_string(),
        }
    }
}

/// The command a restore types into the primary pane, and whether it resumes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Plan {
    SkipLive,
    Resume(String),
    /// Fresh conversation; claude still gets a new pre-assigned id to record.
    Fresh { command: String, reason: &'static str },
}

/// Resume command for `agent` on conversation `id`: the `resume` template of
/// `[agents.<agent>]` with `{id}` replaced, else the built-in default on the configured
/// executable. `None` when the agent has no resume (pi, `resume = ""`, unknown agent).
pub fn resume_command(agents: &AgentsConfig, agent: &str, id: &str) -> Option<String> {
    let product = normalize_client_name(agent);
    let entry = agent_entry(agents, product)?;
    let template = match &entry.resume {
        Some(t) => t.clone(),
        None => {
            let cmd = resolve_agent_command(agents, product).ok()?;
            match product {
                "claude" => format!("{cmd} --resume {{id}}"),
                "cursor" => format!("{cmd} --resume={{id}}"),
                "opencode" => format!("{cmd} --session {{id}}"),
                _ => return None,
            }
        }
    };
    (!template.trim().is_empty()).then(|| template.replace("{id}", id))
}

/// The id is typed into a shell: accept only plain id characters (uuids, `ses_…`).
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn plan(rec: &SessionRecord, agents: &AgentsConfig, live: bool) -> Result<Plan, SpawnError> {
    if live {
        return Ok(Plan::SkipLive);
    }
    if rec.role == "shell" || rec.agent == "shell" {
        return Ok(Plan::Resume(default_shell_command()));
    }
    let fresh = resolve_agent_command(agents, &rec.agent)?;
    let id = rec.agent_session_id.as_deref().filter(|id| safe_id(id));
    Ok(match id {
        None => Plan::Fresh { command: fresh, reason: "no id" },
        Some(id) => match resume_command(agents, &rec.agent, id) {
            Some(cmd) => Plan::Resume(cmd),
            None => Plan::Fresh { command: fresh, reason: "no resume command" },
        },
    })
}

/// Recreate `rec`'s tmux session (same name, cwd, layout, agent / role / parent options)
/// unless one with that name is live. Never switches the client. Missing cwd →
/// `Err` and the record is kept. A fresh claude conversation gets a new `--session-id`
/// that replaces the stale one in the record.
pub fn restore_one(rec: &SessionRecord, agents: &AgentsConfig) -> Result<RestoreOutcome, RestoreError> {
    let live = tmux_has_session(&rec.name).map_err(|_| RestoreError::LiveCheck)?;
    Ok(match plan(rec, agents, live)? {
        Plan::SkipLive => RestoreOutcome::SkippedLive,
        Plan::Resume(cmd) => RestoreOutcome::Restored(respawn_recorded(rec, &cmd)?),
        Plan::Fresh { command, reason } => {
            let (command, new_id) = with_claude_session_id(&command, Some(&rec.agent));
            let rec = SessionRecord {
                agent_session_id: new_id,
                ..rec.clone()
            };
            RestoreOutcome::Fresh {
                reason,
                spawn: respawn_recorded(&rec, &command)?,
            }
        }
    })
}

/// Records with no live tmux session of that name, parents before children.
pub fn dormant(records: Vec<SessionRecord>, live: &HashSet<String>) -> Vec<SessionRecord> {
    parents_first(records.into_iter().filter(|r| !live.contains(&r.name)).collect())
}

/// Stable sort so a fork parent present in `records` comes before its children
/// (its `@cdamux_parent` then names a live session again).
pub fn parents_first(mut records: Vec<SessionRecord>) -> Vec<SessionRecord> {
    let parent_of: HashMap<String, Option<String>> =
        records.iter().map(|r| (r.name.clone(), r.parent.clone())).collect();
    let depth = |name: &str| {
        let mut d = 0;
        let mut cur = parent_of.get(name).cloned().flatten();
        // Bounded walk: a parent cycle in hand-edited files must not hang.
        while let Some(p) = cur.filter(|_| d < parent_of.len()) {
            d += 1;
            cur = parent_of.get(&p).cloned().flatten();
        }
        d
    };
    records.sort_by_cached_key(|r| depth(&r.name));
    records
}

/// True when `agents/<id>.json` says `role=worker` and `parent` is in `parent_ids`.
fn agent_is_worker_of(cdamux_id: &str, parent_ids: &HashSet<String>) -> bool {
    match state_get(StateKind::Agents, cdamux_id) {
        Ok(Some(v)) => {
            v.get("role").and_then(|r| r.as_str()) == Some("worker")
                && v
                    .get("parent")
                    .and_then(|p| p.as_str())
                    .is_some_and(|p| parent_ids.contains(p))
        }
        _ => false,
    }
}

/// When registry `parent` is missing, fill it from agent-state `parent` → host so
/// `parents_first` and `@cdamux_parent` on restore still nest under the orch.
fn fill_missing_host_parents(records: &mut [SessionRecord]) {
    let host_of: HashMap<String, String> = records
        .iter()
        .filter_map(|r| r.cdamux_id.clone().map(|id| (id, r.name.clone())))
        .collect();
    for r in records.iter_mut() {
        if r.parent.is_some() {
            continue;
        }
        let Some(id) = r.cdamux_id.as_deref() else {
            continue;
        };
        if let Ok(Some(v)) = state_get(StateKind::Agents, id) {
            if let Some(pid) = v.get("parent").and_then(|p| p.as_str()) {
                if let Some(host) = host_of.get(pid) {
                    r.parent = Some(host.clone());
                }
            }
        }
    }
}

/// `root` plus its descendants: registry `parent` session names, and workers whose
/// agent `parent` is an agent id already in the set (ADR-011). Parents first.
pub fn restore_subtree(root: SessionRecord, all: Vec<SessionRecord>) -> Vec<SessionRecord> {
    let mut wanted: HashSet<String> = HashSet::from([root.name.clone()]);
    let mut agent_ids: HashSet<String> = root.cdamux_id.iter().cloned().collect();
    // Fixpoint: registry children of wanted hosts + agent-linked workers.
    loop {
        let before = wanted.len();
        for r in &all {
            if wanted.contains(&r.name) {
                if let Some(id) = &r.cdamux_id {
                    agent_ids.insert(id.clone());
                }
                continue;
            }
            let by_host = r.parent.as_ref().is_some_and(|p| wanted.contains(p));
            let by_agent = r
                .cdamux_id
                .as_deref()
                .is_some_and(|id| agent_is_worker_of(id, &agent_ids));
            if by_host || by_agent {
                wanted.insert(r.name.clone());
                if let Some(id) = &r.cdamux_id {
                    agent_ids.insert(id.clone());
                }
            }
        }
        if wanted.len() == before {
            break;
        }
    }
    let mut out: Vec<SessionRecord> = all.into_iter().filter(|r| wanted.contains(&r.name)).collect();
    if !out.iter().any(|r| r.name == root.name) {
        out.push(root);
    }
    fill_missing_host_parents(&mut out);
    parents_first(out)
}

/// Records `cdamux restore <name>` should touch: an orchestrator expands to its
/// worker subtree; anything else is the single record.
pub fn restore_targets(name: &str) -> Option<Vec<SessionRecord>> {
    let root = session_registry::load(name)?;
    if root.role == "orchestrator" {
        Some(restore_subtree(root, session_registry::load_all()))
    } else {
        Some(vec![root])
    }
}

/// `@cdamux_state` of an agent mid-turn: `cdamux restart` leaves it alone without `--force`.
pub fn is_busy_state(raw: Option<&str>) -> bool {
    raw.is_some_and(|s| matches!(s.trim().to_ascii_lowercase().as_str(), "running" | "active" | "working" | "busy"))
}

/// Busy when transitional `@cdamux_state` says so, or when `agents/<id>.json` reports
/// `working` (hooks write there; ADR-011 reported-wins).
fn is_busy_session(info: &SessionPaneInfo, record_id: Option<&str>) -> bool {
    if is_busy_state(info.hook_state.as_deref()) {
        return true;
    }
    let id = info.cdamux_id.as_deref().or(record_id);
    id.and_then(load_reported_status)
        .is_some_and(|s| s == SessionState::Working)
}

/// What `cdamux restart` does with each live record of `records` (dormant ones are
/// dropped): `Ok` = kill then [`restore_one`], `Err` = skip reason. Parents come first
/// and `current` (the client's session) last, so the client keeps its session the longest.
/// `live` is [`crate::list_panes_all`]: live sessions with their raw `@cdamux_state`.
pub fn restart_plan(
    records: Vec<SessionRecord>,
    live: &HashMap<String, SessionPaneInfo>,
    current: Option<&str>,
    force: bool,
) -> Vec<(SessionRecord, Result<(), &'static str>)> {
    let mut plan: Vec<_> = parents_first(records.into_iter().filter(|r| live.contains_key(&r.name)).collect())
        .into_iter()
        .map(|r| {
            let info = &live[&r.name];
            let step = if is_home_session(&r.name) {
                Err("home session")
            } else if is_nav_session(&r.name) {
                Err("navigator session")
            } else if r.role != "agent" || r.agent == "shell" {
                Err("shell session")
            } else if !force && is_busy_session(info, r.cdamux_id.as_deref()) {
                Err("busy; --force restarts it")
            } else {
                Ok(())
            };
            (r, step)
        })
        .collect();
    plan.sort_by_key(|(r, _)| current == Some(r.name.as_str()));
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::spawn::PaneLayout;
    use crate::state::{initial_agent_value, state_append, state_set, StateKind};
    use crate::AgentEntry;

    fn entry(command: &str, resume: Option<&str>) -> Option<AgentEntry> {
        Some(AgentEntry {
            command: command.into(),
            resume: resume.map(str::to_string),
        })
    }

    fn agents() -> AgentsConfig {
        AgentsConfig {
            default: "claude".into(),
            cursor: entry("cursor-agent", None),
            opencode: entry("", None),
            claude: entry("/opt/bin/claude --model opus", None),
            pi: entry("pi", None),
        }
    }

    fn rec(name: &str, agent: &str, id: Option<&str>, parent: Option<&str>) -> SessionRecord {
        SessionRecord {
            name: name.into(),
            cwd: "/tmp".into(),
            agent: agent.into(),
            agent_session_id: id.map(str::to_string),
            cdamux_id: None,
            parent: parent.map(str::to_string),
            role: if agent == "shell" { "shell" } else { "agent" }.into(),
            layout: PaneLayout::Single,
            created_ts: 1,
            updated_ts: 1,
        }
    }

    #[test]
    fn resume_command_defaults_custom_and_none() {
        let a = agents();
        // Default keeps the configured executable and its flags.
        assert_eq!(
            resume_command(&a, "claude", "u1").as_deref(),
            Some("/opt/bin/claude --model opus --resume u1")
        );
        assert_eq!(resume_command(&a, "cursor", "c1").as_deref(), Some("cursor-agent --resume=c1"));
        // Empty command → agent id as executable.
        assert_eq!(resume_command(&a, "opencode", "s1").as_deref(), Some("opencode --session s1"));
        assert_eq!(resume_command(&a, "pi", "x"), None);
        assert_eq!(resume_command(&a, "nope", "x"), None);

        let mut a = agents();
        a.claude = entry("claude", Some("cl -r {id} --x"));
        a.cursor = entry("cursor-agent", Some(""));
        a.pi = entry("pi", Some("pi --continue {id}"));
        assert_eq!(resume_command(&a, "claude", "u1").as_deref(), Some("cl -r u1 --x"));
        assert_eq!(resume_command(&a, "cursor", "c1"), None);
        assert_eq!(resume_command(&a, "pi", "p1").as_deref(), Some("pi --continue p1"));
    }

    #[test]
    fn plan_resume_fresh_shell_and_live() {
        let a = agents();
        let p = |r: &SessionRecord, live| plan(r, &a, live).unwrap();
        assert_eq!(p(&rec("c", "claude", Some("u1"), None), true), Plan::SkipLive);
        assert_eq!(
            p(&rec("c", "claude", Some("u1"), None), false),
            Plan::Resume("/opt/bin/claude --model opus --resume u1".into())
        );
        assert_eq!(
            p(&rec("c", "cursor", None, None), false),
            Plan::Fresh { command: "cursor-agent".into(), reason: "no id" }
        );
        // An id that could inject shell syntax is treated as missing.
        assert_eq!(
            p(&rec("c", "cursor", Some("x; rm -rf ~"), None), false),
            Plan::Fresh { command: "cursor-agent".into(), reason: "no id" }
        );
        assert_eq!(
            p(&rec("c", "pi", Some("p1"), None), false),
            Plan::Fresh { command: "pi".into(), reason: "no resume command" }
        );
        assert_eq!(p(&rec("c", "shell", None, None), false), Plan::Resume(default_shell_command()));
        assert!(matches!(
            plan(&rec("c", "gone", Some("x"), None), &a, false),
            Err(SpawnError::UnknownAgent(_))
        ));
    }

    #[test]
    fn restart_plan_skips_busy_shell_home_and_dormant_current_last() {
        let state = |s: Option<&str>| SessionPaneInfo { hook_state: s.map(str::to_string), ..Default::default() };
        let live: HashMap<String, SessionPaneInfo> = [
            ("a-cur", state(None)),
            ("b-busy", state(Some("running"))),
            ("c-idle", state(Some("idle"))),
            ("d-shell", state(None)),
            ("cdamux", state(None)),
            ("e-child", state(Some("waiting"))),
        ]
        .into_iter()
        .map(|(n, s)| (n.to_string(), s))
        .collect();
        let records = vec![
            rec("a-cur", "claude", None, None),
            rec("b-busy", "claude", None, None),
            rec("c-idle", "cursor", None, None),
            rec("d-shell", "shell", None, None),
            rec("cdamux", "claude", None, None),
            rec("e-child", "claude", None, Some("c-idle")),
            rec("f-dormant", "claude", None, None),
        ];
        let steps = |force| -> Vec<(String, Result<(), &'static str>)> {
            restart_plan(records.clone(), &live, Some("a-cur"), force)
                .into_iter()
                .map(|(r, s)| (r.name, s))
                .collect()
        };
        let plan = steps(false);
        let names: Vec<&str> = plan.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!names.contains(&"f-dormant"));
        assert_eq!(names.last(), Some(&"a-cur"), "current session last");
        let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
        assert!(pos("c-idle") < pos("e-child"), "parents first");
        let step = |p: &[(String, Result<(), &'static str>)], n: &str| p.iter().find(|(x, _)| x == n).unwrap().1;
        assert_eq!(step(&plan, "b-busy"), Err("busy; --force restarts it"));
        assert_eq!(step(&plan, "d-shell"), Err("shell session"));
        assert_eq!(step(&plan, "cdamux"), Err("home session"));
        assert_eq!(step(&plan, "c-idle"), Ok(()));
        assert_eq!(step(&plan, "e-child"), Ok(()));
        let forced = steps(true);
        assert_eq!(step(&forced, "b-busy"), Ok(()));
        assert_eq!(step(&forced, "cdamux"), Err("home session"), "--force never kills home");
        assert!(is_busy_state(Some(" Working ")) && !is_busy_state(Some("done")) && !is_busy_state(None));
    }

    #[test]
    fn restart_plan_skips_reported_working_without_hook_state() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let id = "a-rep1";
        state_set(
            StateKind::Agents,
            id,
            &initial_agent_value(id, "claude", "s-rep", "agent", None),
        )
        .unwrap();
        state_append(
            StateKind::Agents,
            id,
            &serde_json::json!({"status": "working"}),
        )
        .unwrap();

        let live: HashMap<String, SessionPaneInfo> = [(
            "s-rep".to_string(),
            SessionPaneInfo {
                hook_state: None,
                cdamux_id: Some(id.into()),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect();
        let mut record = rec("s-rep", "claude", None, None);
        record.cdamux_id = Some(id.into());
        let plan = restart_plan(vec![record.clone()], &live, None, false);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].1, Err("busy; --force restarts it"));
        assert_eq!(
            restart_plan(vec![record], &live, None, true)[0].1,
            Ok(())
        );
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn dormant_skips_live_and_orders_parents_first() {
        let records = vec![
            rec("a-child", "claude", None, Some("z-root")),
            rec("b-grandchild", "claude", None, Some("a-child")),
            rec("live", "claude", None, None),
            rec("z-root", "claude", None, None),
            rec("orphan", "claude", None, Some("gone")),
            rec("cyc1", "claude", None, Some("cyc2")),
            rec("cyc2", "claude", None, Some("cyc1")),
        ];
        let live: HashSet<String> = ["live".to_string()].into();
        let names: Vec<String> = dormant(records, &live).into_iter().map(|r| r.name).collect();
        let pos = |n: &str| names.iter().position(|x| x == n).unwrap();
        assert!(!names.contains(&"live".to_string()));
        assert_eq!(names.len(), 6);
        assert!(pos("z-root") < pos("a-child") && pos("a-child") < pos("b-grandchild"));
    }

    #[test]
    fn restore_subtree_by_registry_parent_orders_orch_then_workers() {
        let orch = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-orch".into()),
            ..rec("c-orch", "claude", Some("oid"), None)
        };
        let w1 = SessionRecord {
            role: "worker".into(),
            cdamux_id: Some("a-w1".into()),
            ..rec("c-w1", "claude", Some("w1"), Some("c-orch"))
        };
        let w2 = SessionRecord {
            role: "worker".into(),
            cdamux_id: Some("a-w2".into()),
            ..rec("c-w2", "claude", Some("w2"), Some("c-orch"))
        };
        let other = rec("c-other", "claude", None, None);
        let names: Vec<String> = restore_subtree(orch.clone(), vec![w2, other, w1, orch])
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names[0], "c-orch");
        assert!(names.contains(&"c-w1".into()) && names.contains(&"c-w2".into()));
        assert!(!names.contains(&"c-other".into()));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn restore_subtree_includes_worker_via_agent_parent() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let orch = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-orch".into()),
            ..rec("c-orch", "claude", None, None)
        };
        // Registry parent missing; agent state still links the worker.
        let worker = SessionRecord {
            role: "worker".into(),
            cdamux_id: Some("a-w".into()),
            ..rec("c-w", "claude", Some("wid"), None)
        };
        state_set(
            StateKind::Agents,
            "a-w",
            &initial_agent_value("a-w", "claude", "c-w", "worker", Some("a-orch")),
        )
        .unwrap();
        let names: Vec<String> = restore_subtree(orch.clone(), vec![worker, orch])
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, vec!["c-orch".to_string(), "c-w".to_string()]);
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn restore_targets_expands_orchestrator_only() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let orch = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-orch".into()),
            ..rec("c-orch", "claude", None, None)
        };
        let worker = SessionRecord {
            role: "worker".into(),
            cdamux_id: Some("a-w".into()),
            ..rec("c-w", "claude", None, Some("c-orch"))
        };
        let plain = rec("c-plain", "claude", None, None);
        session_registry::record(&orch).unwrap();
        session_registry::record(&worker).unwrap();
        session_registry::record(&plain).unwrap();
        let orch_names: Vec<String> = restore_targets("c-orch")
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(orch_names, vec!["c-orch".to_string(), "c-w".to_string()]);
        assert_eq!(
            restore_targets("c-plain")
                .unwrap()
                .into_iter()
                .map(|r| r.name)
                .collect::<Vec<_>>(),
            vec!["c-plain".to_string()]
        );
        assert!(restore_targets("missing").is_none());
        std::env::remove_var("CDAMUX_HOME");
    }
}
