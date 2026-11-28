//! Orchestrator session entrypoint (`cdamux orch new`).

use crate::client_name::normalize_client_name;
use crate::paths::cdamux_home;
use crate::send::tmux_has_session;
use crate::session_manage::unique_session_name;
use crate::spawn::{
    resolve_agent_command, session_name_for_path, spawn_tmux_session_ex, PaneLayout, SpawnError,
    SpawnExtra, SpawnOutcome,
};
use crate::state::{generate_run_id, state_append, state_list, state_set, StateKind};
use crate::worker::{with_first_prompt, write_first_prompt};
use crate::AgentsConfig;
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Kit skill name the orchestrator client should load (after `install.sh`).
pub const ORCHESTRATOR_SKILL: &str = "cda-orchestrator";

/// Agent `scope` value for the cross-project orchestrator (ADR-015).
pub const ORCH_SCOPE_GLOBAL: &str = "global";

/// `scope` reported for a per-project orchestrator (no `scope` key in its record).
pub const ORCH_SCOPE_PROJECT: &str = "project";

/// First-turn boot text referencing [`ORCHESTRATOR_SKILL`] (per-project).
pub fn orch_boot_prompt() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../prompts/orch-boot.md"
    ))
    .trim()
}

/// First-turn boot text for `cdamux orch new --global`.
pub fn orch_boot_prompt_global() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../prompts/orch-boot-global.md"
    ))
    .trim()
}

/// Boot prompt for the requested scope.
pub fn orch_boot_prompt_for(global: bool) -> &'static str {
    if global {
        orch_boot_prompt_global()
    } else {
        orch_boot_prompt()
    }
}

/// `$CDAMUX_HOME/global` — cwd for a global orchestrator (created if missing).
pub fn global_orch_cwd() -> Result<PathBuf, OrchError> {
    let home = cdamux_home().ok_or(OrchError::NoHome)?;
    let dir = home.join("global");
    std::fs::create_dir_all(&dir).map_err(|e| OrchError::Io(e.to_string()))?;
    Ok(dir)
}

/// `host` of agent `id` when it names a live tmux session (non-empty + `has-session`)
/// that is still its own: a session whose `@cdamux_id` names another agent was reused
/// by a newer one, and this stale record must not look live (ghost orchestrators).
/// Shared liveness rule for [`find_live_global_orchestrator`] and [`list_orchestrators`].
fn live_host<'a>(id: &str, v: &'a serde_json::Value) -> Option<&'a str> {
    let host = v.get("host").and_then(|h| h.as_str()).filter(|h| !h.is_empty())?;
    if !tmux_has_session(host).unwrap_or(false) {
        return None;
    }
    match crate::tmux::tmux_user_option(host, "@cdamux_id") {
        Some(owner) if !owner.is_empty() && owner != id => None,
        _ => Some(host),
    }
}

fn is_orchestrator(v: &serde_json::Value) -> bool {
    v.get("role").and_then(|r| r.as_str()) == Some("orchestrator")
}

fn is_global_scope(v: &serde_json::Value) -> bool {
    v.get("scope").and_then(|s| s.as_str()) == Some(ORCH_SCOPE_GLOBAL)
}

/// Live agent with `role=orchestrator` and `scope=global`, if any.
pub fn find_live_global_orchestrator() -> Option<(String, String)> {
    state_list(StateKind::Agents)
        .into_iter()
        .filter(|(_, _, v)| is_orchestrator(v) && is_global_scope(v))
        .find_map(|(id, _, v)| live_host(&id, &v).map(|h| (id, h.to_string())))
}

/// One recorded orchestrator for `cdamux orch ls` (#388).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrchEntry {
    pub id: String,
    /// Recorded tmux session; empty for a stale record.
    pub host: String,
    /// `global` or `project` (ADR-015).
    pub scope: &'static str,
    /// Project root from the orchestrator's run (`runs/<run>.json`); none for `global`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Reported agent status; `null` when never reported.
    pub status: Option<String>,
    /// Same rule as [`find_live_global_orchestrator`]; stale records stay listed with `false`.
    pub live: bool,
    /// Global orchestrator agent id of a nested one (`orch new --child`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Every `role=orchestrator` agent record, live or not: global first, then by id.
pub fn list_orchestrators() -> Vec<OrchEntry> {
    let mut out: Vec<OrchEntry> = state_list(StateKind::Agents)
        .into_iter()
        .filter(|(_, _, v)| is_orchestrator(v))
        .map(|(id, _, v)| {
            let global = is_global_scope(&v);
            let repo = (!global)
                .then(|| str_field(&v, "run"))
                .flatten()
                .and_then(|run| crate::state::state_get(StateKind::Runs, &run).ok().flatten())
                .and_then(|r| str_field(&r, "repo"));
            OrchEntry {
                host: str_field(&v, "host").unwrap_or_default(),
                scope: if global { ORCH_SCOPE_GLOBAL } else { ORCH_SCOPE_PROJECT },
                repo,
                status: str_field(&v, "status"),
                live: live_host(&id, &v).is_some(),
                parent: str_field(&v, "parent"),
                id,
            }
        })
        .collect();
    out.sort_by(|a, b| (a.scope != ORCH_SCOPE_GLOBAL, &a.id).cmp(&(b.scope != ORCH_SCOPE_GLOBAL, &b.id)));
    out
}

/// Live top-level project orchestrator whose run repo is `path` (same canonical dir).
fn live_orchestrator_for_path(path: &Path) -> Option<OrchEntry> {
    let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    list_orchestrators().into_iter().find(|e| {
        e.live
            && e.scope == ORCH_SCOPE_PROJECT
            && e.parent.is_none()
            && e.repo.as_deref().is_some_and(|r| {
                std::fs::canonicalize(r).unwrap_or_else(|_| PathBuf::from(r)) == want
            })
    })
}

/// `scope` of an orchestrator agent record (`global` or `project`), for `cdamux list --json`.
pub fn orch_scope_of_agent(id: &str) -> Option<&'static str> {
    let v = crate::state::state_get(StateKind::Agents, id).ok().flatten()?;
    is_orchestrator(&v).then(|| if is_global_scope(&v) { ORCH_SCOPE_GLOBAL } else { ORCH_SCOPE_PROJECT })
}

/// True when agent state for `id` has `scope=global`.
pub fn agent_scope_is_global(id: &str) -> bool {
    state_get_scope(id).as_deref() == Some(ORCH_SCOPE_GLOBAL)
}

fn state_get_scope(id: &str) -> Option<String> {
    let v = crate::state::state_get(StateKind::Agents, id).ok().flatten()?;
    v.get("scope")
        .and_then(|s| s.as_str())
        .map(str::to_string)
}

#[derive(Debug, Error)]
pub enum OrchError {
    #[error("{0}")]
    Spawn(#[from] SpawnError),
    #[error("cannot mint run id")]
    NoRunId,
    #[error("CDAMUX_HOME unset")]
    NoHome,
    #[error("io: {0}")]
    Io(String),
    #[error("{0}")]
    Invalid(String),
    /// ADR-015: only a `scope=global` orchestrator may spawn a project orchestrator.
    #[error("{0}")]
    Depth(String),
    /// Q8: a live orchestrator already runs there.
    #[error("orchestrator {id} already live on {host} for {path}; attach to it (cdamux attach {host})")]
    Exists { id: String, host: String, path: String },
    #[error("caller agent '{0}' not found")]
    CallerNotFound(String),
    #[error("caller has no host session")]
    NoHost,
}

#[derive(Debug, Clone)]
pub struct OrchNewRequest<'a> {
    pub path: &'a Path,
    /// Product client override; config default when `None`.
    pub agent: Option<&'a str>,
    pub agents: &'a AgentsConfig,
    pub switch_to_session: bool,
    /// Cross-project orchestrator (`scope=global`, cwd `$CDAMUX_HOME/global`).
    pub global: bool,
    /// Nest under this global orchestrator agent id (`orch new --child`).
    pub parent_agent_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct OrchNewResult {
    pub agent_id: String,
    pub host: String,
    pub run: String,
    pub client: String,
    pub outcome: SpawnOutcome,
    pub path: PathBuf,
    /// True when an existing live global orchestrator was reused.
    pub reused: bool,
}

/// Shared spawn args for `cdamux orch new` and the Fleet spawn-modal orchestrator role.
pub fn spawn_orchestrator(req: OrchNewRequest<'_>) -> Result<OrchNewResult, OrchError> {
    if req.global && req.parent_agent_id.is_some() {
        return Err(OrchError::Invalid(
            "--child is incompatible with --global".into(),
        ));
    }

    // ADR-015 depth ≤ 2: only the global orchestrator may spawn a project orchestrator.
    let parent_host = if let Some(parent_id) = req.parent_agent_id {
        if !agent_scope_is_global(parent_id) {
            return Err(OrchError::Depth(
                "only a scope=global orchestrator may spawn a project orchestrator (depth ≤ 2: global → project → workers)".into(),
            ));
        }
        let parent = crate::state::state_get(StateKind::Agents, parent_id)
            .map_err(|e| OrchError::Io(e.to_string()))?
            .ok_or_else(|| OrchError::CallerNotFound(parent_id.to_string()))?;
        let host = parent
            .get("host")
            .and_then(|h| h.as_str())
            .filter(|s| !s.is_empty())
            .ok_or(OrchError::NoHost)?
            .to_string();
        Some(host)
    } else {
        None
    };

    if req.global {
        if let Some((agent_id, host)) = find_live_global_orchestrator() {
            if req.switch_to_session {
                let _ = crate::session_manage::switch_client_to_session(&host);
            }
            let run = crate::state::state_get(StateKind::Agents, &agent_id)
                .ok()
                .flatten()
                .and_then(|v| v.get("run").and_then(|r| r.as_str()).map(str::to_string))
                .unwrap_or_default();
            let client = crate::state::state_get(StateKind::Agents, &agent_id)
                .ok()
                .flatten()
                .and_then(|v| v.get("client").and_then(|c| c.as_str()).map(str::to_string))
                .unwrap_or_default();
            let path = global_orch_cwd().unwrap_or_else(|_| req.path.to_path_buf());
            return Ok(OrchNewResult {
                agent_id,
                host,
                run,
                client,
                outcome: SpawnOutcome::Created,
                path,
                reused: true,
            });
        }
    }

    let path: PathBuf = if req.global {
        global_orch_cwd()?
    } else {
        req.path.to_path_buf()
    };

    // Q8: one live top-level orchestrator per project; a nested one (`--child`) is the
    // global orchestrator's call (ADR-015).
    if !req.global && req.parent_agent_id.is_none() {
        if let Some(e) = live_orchestrator_for_path(&path) {
            return Err(OrchError::Exists {
                id: e.id,
                host: e.host,
                path: path.display().to_string(),
            });
        }
    }

    let client = req
        .agent
        .map(normalize_client_name)
        .map(str::to_string)
        .unwrap_or_else(|| normalize_client_name(&req.agents.default).to_string());

    let run = generate_run_id().ok_or(OrchError::NoRunId)?;
    let base_cmd = resolve_agent_command(req.agents, &client)?;
    let base_name = if req.global {
        "c-global".to_string()
    } else {
        session_name_for_path(&path)
    };
    let host = unique_session_name(&base_name);
    let boot = orch_boot_prompt_for(req.global);
    let prompt_file = write_first_prompt(&host, boot);
    let (launch_cmd, paste_after) = with_first_prompt(
        &base_cmd,
        &client,
        Some(boot),
        prompt_file.as_deref(),
    );
    let extra = SpawnExtra {
        role: Some("orchestrator"),
        run: Some(run.as_str()),
        parent_agent_id: req.parent_agent_id,
        name: None,
    };
    let outcome = spawn_tmux_session_ex(
        &host,
        &path,
        &launch_cmd,
        PaneLayout::Single,
        req.switch_to_session,
        Some(client.as_str()),
        parent_host.as_deref(),
        extra,
    )?;

    let agent_id = crate::worker::resolve_spawned_agent_id(&host);

    if !agent_id.is_empty() {
        let _ = state_set(
            StateKind::Runs,
            &run,
            &json!({
                "id": run,
                "parent": agent_id,
                "repo": path.display().to_string(),
            }),
        );
        if req.global {
            let _ = state_append(
                StateKind::Agents,
                &agent_id,
                &json!({ "scope": ORCH_SCOPE_GLOBAL }),
            );
        }
    }

    if paste_after {
        crate::send::send_to_session(&host, boot, true)
            .map_err(|e| OrchError::Io(e.to_string()))?;
    }

    Ok(OrchNewResult {
        agent_id,
        host,
        run,
        client,
        outcome,
        path,
        reused: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_prompt_names_skill() {
        let t = orch_boot_prompt();
        assert!(t.contains(ORCHESTRATOR_SKILL), "{t}");
        assert!(t.contains("orchestrator"), "{t}");
        assert!(t.contains("AskQuestion"), "{t}");
        assert!(t.contains("Fil text"), "{t}");
    }

    #[test]
    fn global_boot_prompt_names_skill_and_scope() {
        let t = orch_boot_prompt_global();
        assert!(t.contains(ORCHESTRATOR_SKILL), "{t}");
        assert!(t.contains("scope=global"), "{t}");
        assert!(t.contains("cdamux projects"), "{t}");
        assert!(t.contains("--repo"), "{t}");
        assert!(t.contains("AskQuestion"), "{t}");
        assert!(t.contains("Fil text"), "{t}");
    }

    #[test]
    fn prompt_for_selects_global() {
        assert_eq!(orch_boot_prompt_for(false), orch_boot_prompt());
        assert_eq!(orch_boot_prompt_for(true), orch_boot_prompt_global());
    }

    #[test]
    fn child_refuses_non_global_parent() {
        let _g = crate::env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let _ = state_set(
            StateKind::Agents,
            "a-proj",
            &json!({
                "id": "a-proj",
                "client": "claude",
                "host": "c-proj",
                "role": "orchestrator",
                "parent": null,
                "run": "r-1"
            }),
        );
        let agents = AgentsConfig {
            default: "claude".into(),
            claude: Some(crate::AgentEntry {
                command: "claude".into(),
                resume: None,
            }),
            cursor: None,
            opencode: None,
            pi: None,
        };
        let cwd = dir.path().join("proj");
        std::fs::create_dir(&cwd).unwrap();
        let err = spawn_orchestrator(OrchNewRequest {
            path: &cwd,
            agent: Some("claude"),
            agents: &agents,
            switch_to_session: false,
            global: false,
            parent_agent_id: Some("a-proj"),
        })
        .expect_err("project orch must not spawn orch");
        assert!(
            matches!(err, OrchError::Depth(_)),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("depth"), "{msg}");
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn child_incompatible_with_global_flag() {
        let agents = AgentsConfig {
            default: "claude".into(),
            claude: None,
            cursor: None,
            opencode: None,
            pi: None,
        };
        let err = spawn_orchestrator(OrchNewRequest {
            path: Path::new("/tmp"),
            agent: None,
            agents: &agents,
            switch_to_session: false,
            global: true,
            parent_agent_id: Some("a-global"),
        })
        .expect_err("--child + --global");
        assert!(matches!(err, OrchError::Invalid(_)), "got {err:?}");
    }
}
