//! Spawn tmux sessions with configured agent commands.

use crate::{AgentEntry, AgentsConfig, AttachMode};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpawnError {
    #[error("unknown agent '{0}' (configure [agents.{0}] in config)")]
    UnknownAgent(String),
    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("tmux new-session failed")]
    TmuxFailed,
    #[error("tmux: {0}")]
    Tmux(String),
}

/// skzv-style session name for interoperability.
pub fn session_name_for_path(path: &Path) -> String {
    let base = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "session".to_string());
    format!("c-{base}")
}

pub fn resolve_agent_command(agents: &AgentsConfig, name: &str) -> Result<String, SpawnError> {
    let entry = match name {
        "cursor" => agents.cursor.as_ref(),
        "opencode" => agents.opencode.as_ref(),
        "claude" => agents.claude.as_ref(),
        "pi" => agents.pi.as_ref(),
        _ => None,
    };
    match entry {
        Some(AgentEntry { command }) => Ok(agent_executable(command, name)),
        None => Err(SpawnError::UnknownAgent(name.to_string())),
    }
}

fn agent_executable(command: &str, agent_id: &str) -> String {
    if command.is_empty() {
        agent_id.to_string()
    } else {
        command.to_string()
    }
}

pub fn default_agent_name(agents: &AgentsConfig) -> String {
    agents.default.clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnOutcome {
    Created,
    /// Duplicate `c-<basename>`: attach/switch to existing session instead of creating another.
    ReusedExisting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneLayout {
    #[default]
    Single,
    Horizontal,
    Vertical,
}

pub fn default_shell_command() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
}

/// Create detached tmux session in `path` running `agent_command`.
pub fn spawn_tmux_session(
    session_name: &str,
    path: &Path,
    agent_command: &str,
    attach_mode: AttachMode,
) -> Result<SpawnOutcome, SpawnError> {
    spawn_tmux_session_with_layout(
        session_name,
        path,
        agent_command,
        PaneLayout::Single,
        attach_mode,
        true,
    )
}

/// Primary pane runs `primary_command`; optional split adds a second pane with a shell.
pub fn spawn_tmux_session_with_layout(
    session_name: &str,
    path: &Path,
    primary_command: &str,
    layout: PaneLayout,
    _attach_mode: AttachMode,
    switch_to_session: bool,
) -> Result<SpawnOutcome, SpawnError> {
    if !path.exists() {
        return Err(SpawnError::PathNotFound(path.to_path_buf()));
    }
    if !path.is_dir() {
        return Err(SpawnError::NotDirectory(path.to_path_buf()));
    }

    if tmux_has_session(session_name) {
        if switch_to_session && std::env::var_os("TMUX").is_some() {
            let _ = Command::new("tmux")
                .args(["switch-client", "-t", session_name])
                .status();
        }
        return Ok(SpawnOutcome::ReusedExisting);
    }

    let shell = default_shell_command();
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session_name,
            "-c",
            &path.to_string_lossy(),
            &shell,
        ])
        .status()
        .map_err(|e| SpawnError::Tmux(e.to_string()))?;

    if !status.success() {
        return Err(SpawnError::TmuxFailed);
    }

    if primary_command != shell {
        let send = Command::new("tmux")
            .args(["send-keys", "-t", session_name, primary_command, "Enter"])
            .status()
            .map_err(|e| SpawnError::Tmux(e.to_string()))?;
        if !send.success() {
            return Err(SpawnError::TmuxFailed);
        }
    }

    if layout != PaneLayout::Single {
        apply_pane_split(session_name, layout)?;
    }

    if switch_to_session && std::env::var_os("TMUX").is_some() {
        let _ = Command::new("tmux")
            .args(["switch-client", "-t", session_name])
            .status();
    }

    Ok(SpawnOutcome::Created)
}

fn apply_pane_split(session_name: &str, layout: PaneLayout) -> Result<(), SpawnError> {
    let flag = match layout {
        PaneLayout::Single => return Ok(()),
        PaneLayout::Horizontal => "-h",
        PaneLayout::Vertical => "-v",
    };
    let target = format!("{}:0", session_name);
    let status = Command::new("tmux")
        .args(["split-window", flag, "-t", &target])
        .status()
        .map_err(|e| SpawnError::Tmux(e.to_string()))?;
    if !status.success() {
        return Err(SpawnError::TmuxFailed);
    }
    let shell = default_shell_command();
    let pane1 = format!("{}:0.1", session_name);
    let send = Command::new("tmux")
        .args(["send-keys", "-t", &pane1, &shell, "Enter"])
        .status()
        .map_err(|e| SpawnError::Tmux(e.to_string()))?;
    if !send.success() {
        return Err(SpawnError::TmuxFailed);
    }
    Ok(())
}

fn tmux_has_session(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Agents with a configured command (for interactive picker).
pub fn configured_agent_ids(agents: &AgentsConfig) -> Vec<String> {
    let mut out = Vec::new();
    for (id, configured) in [
        ("cursor", agents.cursor.is_some()),
        ("opencode", agents.opencode.is_some()),
        ("claude", agents.claude.is_some()),
        ("pi", agents.pi.is_some()),
    ] {
        if configured {
            out.push(id.to_string());
        }
    }
    if out.is_empty() {
        out.push(agents.default.clone());
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentsConfig;

    #[test]
    fn pane_layout_cycles_default() {
        assert_eq!(PaneLayout::default(), PaneLayout::Single);
    }

    #[test]
    fn empty_command_uses_agent_id() {
        let agents = AgentsConfig {
            default: "claude".into(),
            cursor: None,
            opencode: None,
            claude: Some(AgentEntry {
                command: String::new(),
            }),
            pi: None,
        };
        assert_eq!(resolve_agent_command(&agents, "claude").unwrap(), "claude");
    }
}
