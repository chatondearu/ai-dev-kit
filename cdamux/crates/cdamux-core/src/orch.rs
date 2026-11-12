//! Orchestrator session entrypoint (`cdamux orch new`).

use crate::client_name::normalize_client_name;
use crate::session_manage::unique_session_name;
use crate::spawn::{
    resolve_agent_command, session_name_for_path, spawn_tmux_session_ex, PaneLayout, SpawnError,
    SpawnExtra, SpawnOutcome,
};
use crate::state::{generate_run_id, state_set, StateKind};
use crate::worker::with_first_prompt;
use crate::AgentsConfig;
use serde_json::json;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Kit skill name the orchestrator client should load (after `install.sh`).
pub const ORCHESTRATOR_SKILL: &str = "cda-orchestrator";

/// First-turn boot text referencing [`ORCHESTRATOR_SKILL`].
pub fn orch_boot_prompt() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../prompts/orch-boot.md"
    ))
    .trim()
}

#[derive(Debug, Error)]
pub enum OrchError {
    #[error("{0}")]
    Spawn(#[from] SpawnError),
    #[error("cannot mint run id")]
    NoRunId,
    #[error("io: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct OrchNewRequest<'a> {
    pub path: &'a Path,
    /// Product client override; config default when `None`.
    pub agent: Option<&'a str>,
    pub agents: &'a AgentsConfig,
    pub switch_to_session: bool,
}

#[derive(Debug, Clone)]
pub struct OrchNewResult {
    pub agent_id: String,
    pub host: String,
    pub run: String,
    pub client: String,
    pub outcome: SpawnOutcome,
    pub path: PathBuf,
}

/// Shared spawn args for `cdamux orch new` and the Fleet spawn-modal orchestrator role.
pub fn spawn_orchestrator(req: OrchNewRequest<'_>) -> Result<OrchNewResult, OrchError> {
    let client = req
        .agent
        .map(normalize_client_name)
        .map(str::to_string)
        .unwrap_or_else(|| normalize_client_name(&req.agents.default).to_string());

    let run = generate_run_id().ok_or(OrchError::NoRunId)?;
    let base_cmd = resolve_agent_command(req.agents, &client)?;
    let (launch_cmd, paste_after) =
        with_first_prompt(&base_cmd, &client, Some(orch_boot_prompt()));

    let host = unique_session_name(&session_name_for_path(req.path));
    let extra = SpawnExtra {
        role: Some("orchestrator"),
        run: Some(run.as_str()),
        parent_agent_id: None,
        name: None,
    };
    let outcome = spawn_tmux_session_ex(
        &host,
        req.path,
        &launch_cmd,
        PaneLayout::Single,
        req.switch_to_session,
        Some(client.as_str()),
        None,
        extra,
    )?;

    let agent_id = crate::session_registry::load(&host)
        .and_then(|r| r.cdamux_id)
        .unwrap_or_default();

    if !agent_id.is_empty() {
        let _ = state_set(
            StateKind::Runs,
            &run,
            &json!({
                "id": run,
                "parent": agent_id,
                "repo": req.path.display().to_string(),
            }),
        );
    }

    if paste_after {
        crate::send::send_to_session(&host, orch_boot_prompt(), true)
            .map_err(|e| OrchError::Io(e.to_string()))?;
    }

    Ok(OrchNewResult {
        agent_id,
        host,
        run,
        client,
        outcome,
        path: req.path.to_path_buf(),
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
}
