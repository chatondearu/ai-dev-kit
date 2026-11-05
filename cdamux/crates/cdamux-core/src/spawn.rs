//! Spawn tmux sessions with configured agent commands.

use crate::session_manage::is_home_session;
use crate::session_registry::{self, now_secs, SessionRecord};
use crate::tmux::pane_target;
use crate::{AgentEntry, AgentsConfig};
use serde::{Deserialize, Serialize};
use std::io::Read;
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
    // tmux itself maps `.` and `:` to `_` (they are target separators).
    format!("c-{}", base.replace(['.', ':'], "_"))
}

pub(crate) fn agent_entry<'a>(agents: &'a AgentsConfig, name: &str) -> Option<&'a AgentEntry> {
    match name {
        "cursor" => agents.cursor.as_ref(),
        "opencode" => agents.opencode.as_ref(),
        "claude" => agents.claude.as_ref(),
        "pi" => agents.pi.as_ref(),
        _ => None,
    }
}

pub fn resolve_agent_command(agents: &AgentsConfig, name: &str) -> Result<String, SpawnError> {
    match agent_entry(agents, name) {
        Some(entry) => Ok(agent_executable(&entry.command, name)),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnOutcome {
    Created,
    /// The session runs but its restore-registry write failed (fail-open).
    CreatedUnrecorded(String),
}

impl SpawnOutcome {
    /// Suffix for a status line: empty, or the registry warning.
    pub fn note(&self) -> String {
        match self {
            Self::Created => String::new(),
            Self::CreatedUnrecorded(e) => format!(" (not recorded for restore: {e})"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneLayout {
    #[default]
    Single,
    Horizontal,
    Vertical,
}

pub fn default_shell_command() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
}

/// Primary pane runs `primary_command`; optional split adds a second pane with a shell.
/// `parent` (fork source) becomes `@cdamux_parent` so the Fleet nests the new session.
/// A labelled session is recorded in the restore registry (ADR-009); `claude` gets a
/// pre-assigned `--session-id` so it can be resumed on the same conversation.
pub fn spawn_tmux_session_with_layout(
    session_name: &str,
    path: &Path,
    primary_command: &str,
    layout: PaneLayout,
    switch_to_session: bool,
    agent_label: Option<&str>,
    parent: Option<&str>,
) -> Result<SpawnOutcome, SpawnError> {
    let (primary_command, agent_session_id) = with_claude_session_id(primary_command, agent_label);
    create_tmux_session(
        session_name,
        path,
        &primary_command,
        layout,
        switch_to_session,
        agent_label,
        parent,
    )?;
    let rec = registry_record(session_name, path, agent_label, parent, layout, agent_session_id);
    Ok(record_outcome(rec))
}

/// Recreate a recorded session as-is (same name, cwd, layout, links) running
/// `primary_command`; the record keeps its `created_ts`, `updated_ts` is bumped.
/// No `--session-id` is added here: the caller built the exact command (restore).
pub(crate) fn respawn_recorded(rec: &SessionRecord, primary_command: &str) -> Result<SpawnOutcome, SpawnError> {
    create_tmux_session(
        &rec.name,
        &rec.cwd,
        primary_command,
        rec.layout,
        false,
        Some(&rec.agent),
        rec.parent.as_deref(),
    )?;
    let mut rec = rec.clone();
    rec.updated_ts = now_secs();
    Ok(record_outcome(Some(rec)))
}

fn record_outcome(rec: Option<SessionRecord>) -> SpawnOutcome {
    match rec.map(|r| session_registry::record(&r)) {
        Some(Err(e)) => SpawnOutcome::CreatedUnrecorded(e.to_string()),
        _ => SpawnOutcome::Created,
    }
}

fn create_tmux_session(
    session_name: &str,
    path: &Path,
    primary_command: &str,
    layout: PaneLayout,
    switch_to_session: bool,
    agent_label: Option<&str>,
    parent: Option<&str>,
) -> Result<(), SpawnError> {
    if !path.exists() {
        return Err(SpawnError::PathNotFound(path.to_path_buf()));
    }
    if !path.is_dir() {
        return Err(SpawnError::NotDirectory(path.to_path_buf()));
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
            .args(["send-keys", "-t", &pane_target(session_name), primary_command, "Enter"])
            .status()
            .map_err(|e| SpawnError::Tmux(e.to_string()))?;
        if !send.success() {
            return Err(SpawnError::TmuxFailed);
        }
    }

    if layout != PaneLayout::Single {
        apply_pane_split(session_name, layout)?;
    }

    for (option, value) in link_options(agent_label, parent) {
        let _ = Command::new("tmux")
            .args(["set-option", "-t", session_name, option, value])
            .status();
    }

    if switch_to_session && std::env::var_os("TMUX").is_some() {
        let _ = Command::new("tmux")
            .args(["switch-client", "-t", session_name])
            .status();
    }
    Ok(())
}

/// `claude` → command plus `--session-id <uuid>`, and that uuid; other agents untouched.
/// No uuid source (no `/dev/urandom`) → no pre-assign rather than a failed spawn.
pub(crate) fn with_claude_session_id(command: &str, agent_label: Option<&str>) -> (String, Option<String>) {
    match (agent_label == Some("claude")).then(uuid_v4).flatten() {
        Some(id) => (format!("{command} --session-id {id}"), Some(id)),
        None => (command.to_string(), None),
    }
}

/// Random RFC 4122 v4 uuid from `/dev/urandom` (no extra crate).
fn uuid_v4() -> Option<String> {
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut b).ok()?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Some(format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]))
}

/// Registry entry for a spawned session; `None` for the home session and unlabelled
/// spawns (nothing to restore). Orch workers never reach this path (orch.sh spawns them).
fn registry_record(
    name: &str,
    path: &Path,
    agent_label: Option<&str>,
    parent: Option<&str>,
    layout: PaneLayout,
    agent_session_id: Option<String>,
) -> Option<SessionRecord> {
    let agent = agent_label?;
    if is_home_session(name) {
        return None;
    }
    let now = now_secs();
    Some(SessionRecord {
        name: name.to_string(),
        cwd: path.to_path_buf(),
        agent: agent.to_string(),
        agent_session_id,
        parent: parent.map(str::to_string),
        role: role_for(agent_label).to_string(),
        layout,
        created_ts: now,
        updated_ts: now,
    })
}

fn role_for(agent_label: Option<&str>) -> &'static str {
    if agent_label == Some("shell") { "shell" } else { "agent" }
}

/// Session user options set at spawn: agent label, role, and the fork parent.
fn link_options<'a>(agent_label: Option<&'a str>, parent: Option<&'a str>) -> Vec<(&'static str, &'a str)> {
    let mut out = Vec::new();
    if let Some(label) = agent_label {
        out.push(("@cdamux_agent", label));
    }
    out.push(("@cdamux_role", role_for(agent_label)));
    if let Some(p) = parent {
        out.push(("@cdamux_parent", p));
    }
    out
}

fn apply_pane_split(session_name: &str, layout: PaneLayout) -> Result<(), SpawnError> {
    let flag = match layout {
        PaneLayout::Single => return Ok(()),
        PaneLayout::Horizontal => "-h",
        PaneLayout::Vertical => "-v",
    };
    // Session target = active pane of the current window (independent of base-index);
    // split-window already starts the default shell in the new pane.
    let status = Command::new("tmux")
        .args(["split-window", flag, "-t", &pane_target(session_name)])
        .status()
        .map_err(|e| SpawnError::Tmux(e.to_string()))?;
    if !status.success() {
        return Err(SpawnError::TmuxFailed);
    }
    Ok(())
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
    fn session_name_replaces_tmux_separators() {
        assert_eq!(session_name_for_path(Path::new("/x/web.app")), "c-web_app");
        assert_eq!(session_name_for_path(Path::new("/x/a:b")), "c-a_b");
    }

    #[test]
    fn fork_sets_parent_and_agent_role() {
        assert_eq!(
            link_options(Some("claude"), Some("c-main")),
            vec![
                ("@cdamux_agent", "claude"),
                ("@cdamux_role", "agent"),
                ("@cdamux_parent", "c-main"),
            ]
        );
        assert_eq!(
            link_options(Some("shell"), None),
            vec![("@cdamux_agent", "shell"), ("@cdamux_role", "shell")]
        );
    }

    #[test]
    fn claude_gets_session_id_others_untouched() {
        let (cmd, id) = with_claude_session_id("claude --x", Some("claude"));
        let id = id.unwrap();
        assert_eq!(cmd, format!("claude --x --session-id {id}"));
        let parts: Vec<usize> = id.split('-').map(str::len).collect();
        assert_eq!(parts, [8, 4, 4, 4, 12]);
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        assert_ne!(Some(id), with_claude_session_id("claude", Some("claude")).1);

        for label in [Some("cursor"), Some("shell"), None] {
            assert_eq!(with_claude_session_id("agent", label), ("agent".into(), None));
        }
    }

    #[test]
    fn registry_record_skips_home_and_unlabelled() {
        let p = Path::new("/tmp/p");
        assert!(registry_record("cdamux", p, Some("claude"), None, PaneLayout::Single, None).is_none());
        assert!(registry_record("c-p", p, None, None, PaneLayout::Single, None).is_none());
        let r = registry_record("c-p", p, Some("shell"), Some("c-main"), PaneLayout::Vertical, None)
            .unwrap();
        assert_eq!(
            (r.agent.as_str(), r.role.as_str(), r.parent.as_deref(), r.layout),
            ("shell", "shell", Some("c-main"), PaneLayout::Vertical)
        );
        assert_eq!(
            registry_record("c-p", p, Some("claude"), None, PaneLayout::Single, Some("u".into()))
                .unwrap()
                .agent_session_id
                .as_deref(),
            Some("u")
        );
    }

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
                resume: None,
            }),
            pi: None,
        };
        assert_eq!(resolve_agent_command(&agents, "claude").unwrap(), "claude");
    }
}
