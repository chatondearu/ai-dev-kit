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
use serde_json::json;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Kit skill name the orchestrator client should load (after `install.sh`).
pub const ORCHESTRATOR_SKILL: &str = "cda-orchestrator";

/// Agent `scope` value for the cross-project orchestrator (ADR-015).
pub const ORCH_SCOPE_GLOBAL: &str = "global";

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

/// Live agent with `role=orchestrator` and `scope=global`, if any.
pub fn find_live_global_orchestrator() -> Option<(String, String)> {
    for (id, _, v) in state_list(StateKind::Agents) {
        if v.get("role").and_then(|r| r.as_str()) != Some("orchestrator") {
            continue;
        }
        if v.get("scope").and_then(|s| s.as_str()) != Some(ORCH_SCOPE_GLOBAL) {
            continue;
        }
        let Some(host) = v.get("host").and_then(|h| h.as_str()).filter(|h| !h.is_empty()) else {
            continue;
        };
        if tmux_has_session(host).unwrap_or(false) {
            return Some((id, host.to_string()));
        }
    }
    None
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
    }

    #[test]
    fn global_boot_prompt_names_skill_and_scope() {
        let t = orch_boot_prompt_global();
        assert!(t.contains(ORCHESTRATOR_SKILL), "{t}");
        assert!(t.contains("scope=global"), "{t}");
        assert!(t.contains("cdamux projects"), "{t}");
        assert!(t.contains("--repo"), "{t}");
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
