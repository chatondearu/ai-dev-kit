//! Restore recorded sessions (ADR-009): recreate a dormant session from its registry
//! record and resume the agent on the same conversation when the agent supports it.

use crate::agent_status::load_reported_status;
use crate::client_name::normalize_client_name;
use crate::msg::msg_send;
use crate::orch::{find_live_global_orchestrator, ORCH_SCOPE_GLOBAL};
use crate::send::tmux_has_session;
use crate::session::SessionState;
use crate::session_manage::{is_home_session, is_nav_session};
use crate::session_registry::{self, SessionRecord};
use crate::spawn::{
    agent_entry, default_shell_command, resolve_agent_command, respawn_recorded,
    with_claude_session_id, SpawnError, SpawnOutcome,
};
use crate::state::{state_append, state_get, StateKind};
use crate::state_gc::is_gc_agent_status;
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
    Fresh {
        reason: &'static str,
        spawn: SpawnOutcome,
    },
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
    Fresh {
        command: String,
        reason: &'static str,
    },
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
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
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
        None => Plan::Fresh {
            command: fresh,
            reason: "no id",
        },
        Some(id) => match resume_command(agents, &rec.agent, id) {
            Some(cmd) => Plan::Resume(cmd),
            None => Plan::Fresh {
                command: fresh,
                reason: "no resume command",
            },
        },
    })
}

/// Recreate `rec`'s tmux session (same name, cwd, layout, agent / role / parent options)
/// unless one with that name is live. Never switches the client. Missing cwd →
/// `Err` and the record is kept. A fresh claude conversation gets a new `--session-id`
/// that replaces the stale one in the record.
pub fn restore_one(
    rec: &SessionRecord,
    agents: &AgentsConfig,
) -> Result<RestoreOutcome, RestoreError> {
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
    parents_first(
        records
            .into_iter()
            .filter(|r| !live.contains(&r.name))
            .collect(),
    )
}

/// Stable sort so a fork parent present in `records` comes before its children
/// (its `@cdamux_parent` then names a live session again).
pub fn parents_first(mut records: Vec<SessionRecord>) -> Vec<SessionRecord> {
    let parent_of: HashMap<String, Option<String>> = records
        .iter()
        .map(|r| (r.name.clone(), r.parent.clone()))
        .collect();
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

/// True when `agents/<id>.json` has `parent` in `parent_ids` and role is worker or
/// nested orchestrator (ADR-015: global → project orch → workers).
fn agent_is_child_of(cdamux_id: &str, parent_ids: &HashSet<String>) -> bool {
    match state_get(StateKind::Agents, cdamux_id) {
        Ok(Some(v)) => {
            let role = v.get("role").and_then(|r| r.as_str());
            let nested = role == Some("worker") || role == Some("orchestrator");
            nested
                && v.get("parent")
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
                .is_some_and(|id| agent_is_child_of(id, &agent_ids));
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
    let mut out: Vec<SessionRecord> = all
        .into_iter()
        .filter(|r| wanted.contains(&r.name))
        .collect();
    if !out.iter().any(|r| r.name == root.name) {
        out.push(root);
    }
    fill_missing_host_parents(&mut out);
    parents_first(out)
}

/// True when the record's agent reported `done` (`cdamux msg --kind done`): its job
/// is over, so bulk restores (`--all`, an orchestrator's subtree) leave it dormant.
pub fn is_done_record(rec: &SessionRecord) -> bool {
    rec.cdamux_id.as_deref().and_then(load_reported_status) == Some(SessionState::Done)
}

/// Registry record of the most recently updated `scope=global` orchestrator, if any.
/// Reboot path (`cdamux restore --last-orch`): restore this host and its resumable workers.
/// Prefers a live global host (ADR-015) over a newer dormant record.
pub fn last_global_orch_record() -> Option<SessionRecord> {
    let live = find_live_global_orchestrator().map(|(_, host)| host);
    pick_global_orch_record(live.as_deref())
}

/// `live_host`: tmux session of a live `scope=global` orch, when one exists.
/// Live host wins even if another global record has a newer `updated_ts`.
pub fn pick_global_orch_record(live_host: Option<&str>) -> Option<SessionRecord> {
    if let Some(host) = live_host.filter(|h| !h.is_empty()) {
        return session_registry::load(host).or_else(|| {
            session_registry::load_all()
                .into_iter()
                .find(|r| r.name == host)
        });
    }
    newest_recorded_global_orch()
}

fn newest_recorded_global_orch() -> Option<SessionRecord> {
    session_registry::load_all()
        .into_iter()
        .filter(|r| r.role == "orchestrator" && record_scope_is_global(r))
        .max_by_key(|r| r.updated_ts)
}

fn record_scope_is_global(rec: &SessionRecord) -> bool {
    rec.cdamux_id
        .as_deref()
        .and_then(|id| state_get(StateKind::Agents, id).ok().flatten())
        .and_then(|v| v.get("scope").and_then(|s| s.as_str()).map(str::to_string))
        .as_deref()
        == Some(ORCH_SCOPE_GLOBAL)
}

/// Records `cdamux restore <name>` should touch: an orchestrator expands to its
/// worker subtree (minus finished workers); anything else is the single record.
pub fn restore_targets(name: &str) -> Option<Vec<SessionRecord>> {
    let root = session_registry::load(name)?;
    if root.role == "orchestrator" {
        let subtree = restore_subtree(root, session_registry::load_all());
        // The named root is restored even when done: the user asked for it by name.
        Some(
            subtree
                .into_iter()
                .filter(|r| r.name == name || !is_done_record(r))
                .collect(),
        )
    } else {
        Some(vec![root])
    }
}

/// `@cdamux_state` of an agent mid-turn: `cdamux restart` leaves it alone without `--force`.
pub fn is_busy_state(raw: Option<&str>) -> bool {
    raw.is_some_and(|s| {
        matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "running" | "active" | "working" | "busy"
        )
    })
}

/// Busy when transitional `@cdamux_state` says so, or when `agents/<id>.json` reports
/// `working` (hooks write there; ADR-011 reported-wins).
fn is_busy_session(info: &SessionPaneInfo, record_id: Option<&str>) -> bool {
    // Agent CLI gone (fish / mise / direnv) for long enough: not busy.
    if pane_is_dead_agent(info) {
        return false;
    }
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
    let mut plan: Vec<_> = parents_first(
        records
            .into_iter()
            .filter(|r| live.contains_key(&r.name))
            .collect(),
    )
    .into_iter()
    .map(|r| {
        let info = &live[&r.name];
        let step = if is_home_session(&r.name) {
            Err("home session")
        } else if is_nav_session(&r.name) {
            Err("navigator session")
        } else if r.role == "shell" || r.agent == "shell" {
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

/// Seconds a shell/`mise` pane must stay that way before we treat the agent as dead.
/// Avoids the spawn race where `#{pane_current_command}` is still `sh` wrapping the CLI.
pub const DEAD_AGENT_PANE_SECS: u32 = 10;

fn pane_is_dead_agent(info: &SessionPaneInfo) -> bool {
    // Foreground comm of fish/mise/direnv is dead even when tmux still reports
    // `cursor-agent` (`has_agent_process` is then already false). The shell that
    // owns `pane_pid` stays `fish` for the whole session, so that comm is not the signal.
    let proc_dead = info
        .proc_comm
        .as_deref()
        .is_some_and(crate::tmux::is_dead_pane_comm);
    let cmd_dead = info.saw_pane_command && !info.has_agent_process;
    (proc_dead || cmd_dead) && info.idle_seconds.is_some_and(|s| s >= DEAD_AGENT_PANE_SECS)
}

/// Recorded agent/worker/orch whose tmux host is live but the pane is only a
/// shell (or `mise` / `direnv`): mark `error` and inbox the parent (nudge).
/// Host is kept so `cdamux restore` / `restart --force` can resume.
/// Fail-open: IO / msg errors skip that row.
pub fn reap_dead_agent_panes(panes: &HashMap<String, SessionPaneInfo>) -> usize {
    let mut n = 0;
    for rec in session_registry::load_all() {
        if rec.role == "shell" || rec.agent == "shell" {
            continue;
        }
        let Some(info) = panes.get(&rec.name) else {
            continue;
        };
        if !pane_is_dead_agent(info) {
            continue;
        }
        let Some(id) = rec.cdamux_id.as_deref() else {
            continue;
        };
        let Ok(Some(v)) = state_get(StateKind::Agents, id) else {
            continue;
        };
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
        if is_gc_agent_status(status) {
            continue;
        }
        // Force error even from sticky `blocked`: the process is gone.
        if state_append(
            StateKind::Agents,
            id,
            &serde_json::json!({"status": "error"}),
        )
        .is_err()
        {
            continue;
        }
        if let Some(parent) = v
            .get("parent")
            .and_then(|p| p.as_str())
            .filter(|p| !p.is_empty())
        {
            let text = format!(
                "agent process gone on {} (pane is shell/mise, CLI exited or mise untrusted). cdamux restore {}",
                rec.name, rec.name
            );
            let _ = msg_send(
                id,
                parent,
                "error",
                &text,
                None,
                None,
            );
        }
        n += 1;
    }
    n
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
        assert_eq!(
            resume_command(&a, "cursor", "c1").as_deref(),
            Some("cursor-agent --resume=c1")
        );
        // Empty command → agent id as executable.
        assert_eq!(
            resume_command(&a, "opencode", "s1").as_deref(),
            Some("opencode --session s1")
        );
        assert_eq!(resume_command(&a, "pi", "x"), None);
        assert_eq!(resume_command(&a, "nope", "x"), None);

        let mut a = agents();
        a.claude = entry("claude", Some("cl -r {id} --x"));
        a.cursor = entry("cursor-agent", Some(""));
        a.pi = entry("pi", Some("pi --continue {id}"));
        assert_eq!(
            resume_command(&a, "claude", "u1").as_deref(),
            Some("cl -r u1 --x")
        );
        assert_eq!(resume_command(&a, "cursor", "c1"), None);
        assert_eq!(
            resume_command(&a, "pi", "p1").as_deref(),
            Some("pi --continue p1")
        );
    }

    #[test]
    fn plan_resume_fresh_shell_and_live() {
        let a = agents();
        let p = |r: &SessionRecord, live| plan(r, &a, live).unwrap();
        assert_eq!(
            p(&rec("c", "claude", Some("u1"), None), true),
            Plan::SkipLive
        );
        assert_eq!(
            p(&rec("c", "claude", Some("u1"), None), false),
            Plan::Resume("/opt/bin/claude --model opus --resume u1".into())
        );
        assert_eq!(
            p(&rec("c", "cursor", None, None), false),
            Plan::Fresh {
                command: "cursor-agent".into(),
                reason: "no id"
            }
        );
        // An id that could inject shell syntax is treated as missing.
        assert_eq!(
            p(&rec("c", "cursor", Some("x; rm -rf ~"), None), false),
            Plan::Fresh {
                command: "cursor-agent".into(),
                reason: "no id"
            }
        );
        assert_eq!(
            p(&rec("c", "pi", Some("p1"), None), false),
            Plan::Fresh {
                command: "pi".into(),
                reason: "no resume command"
            }
        );
        assert_eq!(
            p(&rec("c", "shell", None, None), false),
            Plan::Resume(default_shell_command())
        );
        assert!(matches!(
            plan(&rec("c", "gone", Some("x"), None), &a, false),
            Err(SpawnError::UnknownAgent(_))
        ));
    }

    #[test]
    fn restart_plan_skips_busy_shell_home_and_dormant_current_last() {
        let state = |s: Option<&str>| SessionPaneInfo {
            hook_state: s.map(str::to_string),
            ..Default::default()
        };
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
        let step = |p: &[(String, Result<(), &'static str>)], n: &str| {
            p.iter().find(|(x, _)| x == n).unwrap().1
        };
        assert_eq!(step(&plan, "b-busy"), Err("busy; --force restarts it"));
        assert_eq!(step(&plan, "d-shell"), Err("shell session"));
        assert_eq!(step(&plan, "cdamux"), Err("home session"));
        assert_eq!(step(&plan, "c-idle"), Ok(()));
        assert_eq!(step(&plan, "e-child"), Ok(()));
        let forced = steps(true);
        assert_eq!(step(&forced, "b-busy"), Ok(()));
        assert_eq!(
            step(&forced, "cdamux"),
            Err("home session"),
            "--force never kills home"
        );
        assert!(
            is_busy_state(Some(" Working "))
                && !is_busy_state(Some("done"))
                && !is_busy_state(None)
        );
    }

    #[test]
    fn restart_plan_restarts_worker_role_and_dead_agent_pane() {
        let live: HashMap<String, SessionPaneInfo> = [
            (
                "c-w",
                SessionPaneInfo {
                    hook_state: Some("working".into()),
                    saw_pane_command: true,
                    has_agent_process: false,
                    idle_seconds: Some(super::DEAD_AGENT_PANE_SECS),
                    ..Default::default()
                },
            ),
            (
                "c-ok",
                SessionPaneInfo {
                    hook_state: Some("working".into()),
                    saw_pane_command: true,
                    has_agent_process: true,
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .map(|(n, s)| (n.to_string(), s))
        .collect();
        let mut worker = rec("c-w", "claude", None, None);
        worker.role = "worker".into();
        let mut live_agent = rec("c-ok", "claude", None, None);
        live_agent.role = "worker".into();
        let plan = restart_plan(vec![worker, live_agent], &live, None, false);
        let step = |n: &str| plan.iter().find(|(r, _)| r.name == n).unwrap().1;
        assert_eq!(
            step("c-w"),
            Ok(()),
            "dead pane is not a shell skip and not busy"
        );
        assert_eq!(step("c-ok"), Err("busy; --force restarts it"));
        assert_eq!(
            restart_plan(vec![rec("c-ok", "claude", None, None)], &live, None, true)[0].1,
            Ok(())
        );
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
        assert_eq!(restart_plan(vec![record], &live, None, true)[0].1, Ok(()));
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
        let names: Vec<String> = dormant(records, &live)
            .into_iter()
            .map(|r| r.name)
            .collect();
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
    fn is_done_record_reads_agent_status() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let with_id = |name: &str, id: &str| SessionRecord {
            cdamux_id: Some(id.into()),
            ..rec(name, "claude", None, None)
        };
        state_set(
            StateKind::Agents,
            "a-done",
            &serde_json::json!({"id": "a-done", "status": "done"}),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-idle",
            &serde_json::json!({"id": "a-idle", "status": "idle"}),
        )
        .unwrap();
        assert!(is_done_record(&with_id("c-done", "a-done")));
        assert!(!is_done_record(&with_id("c-idle", "a-idle")));
        assert!(!is_done_record(&with_id("c-none", "a-missing")));
        assert!(!is_done_record(&rec("c-noid", "claude", None, None)));
        std::env::remove_var("CDAMUX_HOME");
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
    fn restore_subtree_two_levels_global_project_orch_worker() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let global = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-global".into()),
            ..rec("c-global", "claude", None, None)
        };
        // Registry parent missing; agent parent links project orch → global, worker → project.
        let project = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-proj".into()),
            ..rec("c-proj", "claude", None, None)
        };
        let worker = SessionRecord {
            role: "worker".into(),
            cdamux_id: Some("a-w".into()),
            ..rec("c-w", "claude", None, None)
        };
        state_set(
            StateKind::Agents,
            "a-proj",
            &initial_agent_value(
                "a-proj",
                "claude",
                "c-proj",
                "orchestrator",
                Some("a-global"),
            ),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-w",
            &initial_agent_value("a-w", "claude", "c-w", "worker", Some("a-proj")),
        )
        .unwrap();
        let names: Vec<String> = restore_subtree(global.clone(), vec![worker, project, global])
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(
            names,
            vec![
                "c-global".to_string(),
                "c-proj".to_string(),
                "c-w".to_string()
            ]
        );
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

    #[test]
    fn last_global_orch_record_picks_newest_global() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let old = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-old".into()),
            updated_ts: 10,
            ..rec("c-old-global", "cursor", Some("sid-old"), None)
        };
        let newest = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-new".into()),
            updated_ts: 50,
            ..rec("c-global", "cursor", Some("sid-new"), None)
        };
        let project = SessionRecord {
            role: "orchestrator".into(),
            cdamux_id: Some("a-proj".into()),
            updated_ts: 99,
            ..rec("c-proj", "cursor", Some("sid-p"), None)
        };
        session_registry::record(&old).unwrap();
        session_registry::record(&newest).unwrap();
        session_registry::record(&project).unwrap();
        state_set(
            StateKind::Agents,
            "a-old",
            &serde_json::json!({"id":"a-old","role":"orchestrator","scope":"global","status":"error","host":""}),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-new",
            &serde_json::json!({"id":"a-new","role":"orchestrator","scope":"global","status":"working","host":"c-global"}),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-proj",
            &serde_json::json!({"id":"a-proj","role":"orchestrator","status":"error","host":""}),
        )
        .unwrap();
        let got = pick_global_orch_record(None).expect("global orch");
        assert_eq!(got.name, "c-global");
        assert_eq!(got.cdamux_id.as_deref(), Some("a-new"));
        let live = pick_global_orch_record(Some("c-old-global")).expect("live wins");
        assert_eq!(live.name, "c-old-global");
        assert_eq!(live.cdamux_id.as_deref(), Some("a-old"));
        assert!(
            pick_global_orch_record(Some("c-missing-live")).is_none(),
            "live host with no registry must not fall back to a newer dormant"
        );
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn reap_dead_agent_panes_marks_error_and_inboxes_parent() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        state_set(
            StateKind::Agents,
            "a-orch-reap",
            &initial_agent_value(
                "a-orch-reap",
                "claude",
                "c-reap-orch-no-tmux",
                "orchestrator",
                None,
            ),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-dead",
            &initial_agent_value(
                "a-dead",
                "cursor",
                "c-fil-diag",
                "worker",
                Some("a-orch-reap"),
            ),
        )
        .unwrap();
        state_append(
            StateKind::Agents,
            "a-dead",
            &serde_json::json!({"status": "working"}),
        )
        .unwrap();
        state_set(
            StateKind::Agents,
            "a-live",
            &initial_agent_value("a-live", "cursor", "c-alive", "worker", Some("a-orch-reap")),
        )
        .unwrap();
        state_append(
            StateKind::Agents,
            "a-live",
            &serde_json::json!({"status": "working"}),
        )
        .unwrap();
        let mut dead = rec("c-fil-diag", "cursor", None, Some("c-reap-orch-no-tmux"));
        dead.role = "worker".into();
        dead.cdamux_id = Some("a-dead".into());
        let mut live = rec("c-alive", "cursor", None, Some("c-reap-orch-no-tmux"));
        live.role = "worker".into();
        live.cdamux_id = Some("a-live".into());
        session_registry::record(&dead).unwrap();
        session_registry::record(&live).unwrap();

        let panes: HashMap<String, SessionPaneInfo> = [
            (
                "c-fil-diag".to_string(),
                SessionPaneInfo {
                    saw_pane_command: true,
                    has_agent_process: false,
                    idle_seconds: Some(DEAD_AGENT_PANE_SECS),
                    ..Default::default()
                },
            ),
            (
                "c-alive".to_string(),
                SessionPaneInfo {
                    saw_pane_command: true,
                    has_agent_process: true,
                    idle_seconds: Some(DEAD_AGENT_PANE_SECS),
                    ..Default::default()
                },
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(reap_dead_agent_panes(&panes), 1);
        let dead_st = state_get(StateKind::Agents, "a-dead").unwrap().unwrap();
        assert_eq!(dead_st["status"], "error");
        let live_st = state_get(StateKind::Agents, "a-live").unwrap().unwrap();
        assert_eq!(live_st["status"], "working");
        let inbox = crate::msg::read_inbox_all("a-orch-reap").unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].kind, "error");
        assert!(inbox[0].text.contains("c-fil-diag"));
        assert_eq!(reap_dead_agent_panes(&panes), 0, "idempotent once error");

        let mut mise = rec("c-mise", "cursor", None, Some("c-reap-orch-no-tmux"));
        mise.role = "worker".into();
        mise.cdamux_id = Some("a-mise".into());
        state_set(
            StateKind::Agents,
            "a-mise",
            &initial_agent_value("a-mise", "cursor", "c-mise", "worker", Some("a-orch-reap")),
        )
        .unwrap();
        state_append(
            StateKind::Agents,
            "a-mise",
            &serde_json::json!({"status": "working"}),
        )
        .unwrap();
        session_registry::record(&mise).unwrap();
        let mut panes = panes;
        panes.insert(
            "c-mise".into(),
            SessionPaneInfo {
                saw_pane_command: true,
                has_agent_process: false,
                idle_seconds: Some(DEAD_AGENT_PANE_SECS),
                ..Default::default()
            },
        );
        // Fresh spawn: pane still `sh` wrapping the CLI — do not reap.
        let mut fresh = rec("c-fresh", "cursor", None, Some("c-reap-orch-no-tmux"));
        fresh.role = "worker".into();
        fresh.cdamux_id = Some("a-fresh".into());
        state_set(
            StateKind::Agents,
            "a-fresh",
            &initial_agent_value(
                "a-fresh",
                "cursor",
                "c-fresh",
                "worker",
                Some("a-orch-reap"),
            ),
        )
        .unwrap();
        state_append(
            StateKind::Agents,
            "a-fresh",
            &serde_json::json!({"status": "working"}),
        )
        .unwrap();
        session_registry::record(&fresh).unwrap();
        panes.insert(
            "c-fresh".into(),
            SessionPaneInfo {
                saw_pane_command: true,
                has_agent_process: false,
                idle_seconds: Some(0),
                ..Default::default()
            },
        );
        assert_eq!(reap_dead_agent_panes(&panes), 1);
        assert_eq!(
            state_get(StateKind::Agents, "a-mise").unwrap().unwrap()["status"],
            "error"
        );
        assert_eq!(
            state_get(StateKind::Agents, "a-fresh").unwrap().unwrap()["status"],
            "working",
            "idle 0 shell pane is spawn, not death"
        );
        std::env::remove_var("CDAMUX_HOME");
    }
}
