//! Spawn tmux sessions with configured agent commands.

use crate::client_name::normalize_client_name;
use crate::session_manage::is_home_session;
use crate::session_registry::{self, now_secs, SessionRecord};
use crate::state::{
    generate_agent_id, initial_agent_value, random_bytes, state_append, state_get, state_set,
    StateKind,
};
use crate::tmux::pane_target;
use crate::{AgentEntry, AgentsConfig};
use serde::{Deserialize, Serialize};
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
    match normalize_client_name(name) {
        "cursor" => agents.cursor.as_ref(),
        "opencode" => agents.opencode.as_ref(),
        "claude" => agents.claude.as_ref(),
        "pi" => agents.pi.as_ref(),
        _ => None,
    }
}

pub fn resolve_agent_command(agents: &AgentsConfig, name: &str) -> Result<String, SpawnError> {
    let product = normalize_client_name(name);
    match agent_entry(agents, product) {
        Some(entry) => Ok(agent_executable(&entry.command, product)),
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

/// Optional role / run / parent-agent overrides for worker and orchestrator spawns.
#[derive(Debug, Clone, Default)]
pub struct SpawnExtra<'a> {
    /// `@cdamux_role` and agent `role` (default: agent/shell from label).
    pub role: Option<&'a str>,
    /// `@cdamux_run` and agent `run`.
    pub run: Option<&'a str>,
    /// Agent-state `parent` (agent id). When set, skips registry lookup of `parent` session.
    pub parent_agent_id: Option<&'a str>,
    /// Optional logical name (worker spawn `<name>`) stored on the agent doc.
    pub name: Option<&'a str>,
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
    spawn_tmux_session_ex(
        session_name,
        path,
        primary_command,
        layout,
        switch_to_session,
        agent_label,
        parent,
        SpawnExtra::default(),
    )
}

/// Like [`spawn_tmux_session_with_layout`] with role / run / parent-agent overrides.
pub fn spawn_tmux_session_ex(
    session_name: &str,
    path: &Path,
    primary_command: &str,
    layout: PaneLayout,
    switch_to_session: bool,
    agent_label: Option<&str>,
    parent: Option<&str>,
    extra: SpawnExtra<'_>,
) -> Result<SpawnOutcome, SpawnError> {
    let agent_label = agent_label.map(normalize_client_name);
    let cdamux_id = agent_label.and_then(|_| generate_agent_id());
    let (primary_command, agent_session_id) =
        with_claude_session_id(primary_command, agent_label);
    let primary_command = with_agent_id_env(&primary_command, cdamux_id.as_deref());
    create_tmux_session(
        session_name,
        path,
        &primary_command,
        layout,
        switch_to_session,
        agent_label,
        parent,
        cdamux_id.as_deref(),
        &extra,
    )?;
    write_agent_state(session_name, agent_label, parent, cdamux_id.as_deref(), &extra);
    let rec = registry_record(
        session_name,
        path,
        agent_label,
        parent,
        layout,
        agent_session_id,
        cdamux_id,
        &extra,
    );
    Ok(record_outcome(rec))
}

/// Recreate a recorded session as-is (same name, cwd, layout, links) running
/// `primary_command`; the record keeps its `created_ts`, `updated_ts` is bumped.
/// No `--session-id` is added here: the caller built the exact command (restore).
pub(crate) fn respawn_recorded(rec: &SessionRecord, primary_command: &str) -> Result<SpawnOutcome, SpawnError> {
    let primary_command = with_agent_id_env(primary_command, rec.cdamux_id.as_deref());
    let extra = SpawnExtra {
        role: Some(rec.role.as_str()),
        ..Default::default()
    };
    create_tmux_session(
        &rec.name,
        &rec.cwd,
        &primary_command,
        rec.layout,
        false,
        Some(&rec.agent),
        rec.parent.as_deref(),
        rec.cdamux_id.as_deref(),
        &extra,
    )?;
    // Merge, do not replace: append-only keys (status, resume ids, …) must survive restore.
    refresh_agent_state_on_restore(
        &rec.name,
        Some(rec.agent.as_str()),
        rec.parent.as_deref(),
        rec.cdamux_id.as_deref(),
        &extra,
    );
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
    cdamux_id: Option<&str>,
    extra: &SpawnExtra<'_>,
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

    if let Some(id) = cdamux_id {
        let _ = Command::new("tmux")
            .args(["set-environment", "-t", session_name, "CDAMUX_AGENT_ID", id])
            .status();
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

    for (option, value) in link_options(agent_label, parent, cdamux_id, extra) {
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

/// Prefix the pane command so the agent process sees `CDAMUX_AGENT_ID` even when the
/// shell was started before `set-environment`.
fn with_agent_id_env(command: &str, cdamux_id: Option<&str>) -> String {
    match cdamux_id {
        Some(id) if !id.is_empty() => format!("CDAMUX_AGENT_ID={id} {command}"),
        _ => command.to_string(),
    }
}

/// Resolve a fork parent's tmux session name to its agent id (ADR-011 §1).
/// Missing registry / id → `None` (persist JSON null until the parent is known).
fn parent_agent_id(parent_session: Option<&str>) -> Option<String> {
    parent_session.and_then(|name| session_registry::load(name)?.cdamux_id)
}

fn write_agent_state(
    session_name: &str,
    agent_label: Option<&str>,
    parent: Option<&str>,
    cdamux_id: Option<&str>,
    extra: &SpawnExtra<'_>,
) {
    let (Some(id), Some(client)) = (cdamux_id, agent_label) else {
        return;
    };
    let role = resolve_role(agent_label, extra);
    let parent_id = extra
        .parent_agent_id
        .map(str::to_string)
        .or_else(|| parent_agent_id(parent));
    let mut value = initial_agent_value(id, client, session_name, role, parent_id.as_deref());
    if let Some(run) = extra.run.filter(|s| !s.is_empty()) {
        value
            .as_object_mut()
            .map(|m| m.insert("run".into(), serde_json::Value::String(run.into())));
    }
    if let Some(name) = extra.name.filter(|s| !s.is_empty()) {
        value
            .as_object_mut()
            .map(|m| m.insert("name".into(), serde_json::Value::String(name.into())));
    }
    let _ = state_set(StateKind::Agents, id, &value);
}

/// On restore: refresh `host` and fill only missing baseline fields so keys added via
/// `cdamux state append` (status, resume ids, …) survive (ADR-011 resilience).
fn refresh_agent_state_on_restore(
    session_name: &str,
    agent_label: Option<&str>,
    parent: Option<&str>,
    cdamux_id: Option<&str>,
    extra: &SpawnExtra<'_>,
) {
    let (Some(id), Some(client)) = (cdamux_id, agent_label) else {
        return;
    };
    let role = resolve_role(agent_label, extra);
    let parent_id = extra
        .parent_agent_id
        .map(str::to_string)
        .or_else(|| parent_agent_id(parent));
    let baseline = initial_agent_value(id, client, session_name, role, parent_id.as_deref());
    match state_get(StateKind::Agents, id) {
        Ok(Some(existing)) => {
            let patch = match (&existing, &baseline) {
                (serde_json::Value::Object(ex), serde_json::Value::Object(base)) => {
                    let mut patch = serde_json::Map::new();
                    if let Some(host) = base.get("host") {
                        patch.insert("host".into(), host.clone());
                    }
                    for (k, v) in base {
                        if k != "host" && !ex.contains_key(k) {
                            patch.insert(k.clone(), v.clone());
                        }
                    }
                    serde_json::Value::Object(patch)
                }
                _ => baseline,
            };
            let _ = state_append(StateKind::Agents, id, &patch);
        }
        _ => {
            let _ = state_set(StateKind::Agents, id, &baseline);
        }
    }
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
    let mut b = random_bytes::<16>()?;
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
    cdamux_id: Option<String>,
    extra: &SpawnExtra<'_>,
) -> Option<SessionRecord> {
    let agent = agent_label?;
    if is_home_session(name) {
        return None;
    }
    let now = now_secs();
    Some(SessionRecord {
        name: name.to_string(),
        cwd: path.to_path_buf(),
        agent: normalize_client_name(agent).to_string(),
        agent_session_id,
        cdamux_id,
        parent: parent.map(str::to_string),
        role: resolve_role(agent_label, extra).to_string(),
        layout,
        created_ts: now,
        updated_ts: now,
    })
}

fn role_for(agent_label: Option<&str>) -> &'static str {
    if agent_label == Some("shell") {
        "shell"
    } else {
        "agent"
    }
}

fn resolve_role<'a>(agent_label: Option<&str>, extra: &SpawnExtra<'a>) -> &'a str {
    extra.role.unwrap_or_else(|| role_for(agent_label))
}

/// Session user options set at spawn: agent label, role, stable id, run, and the fork parent.
fn link_options<'a>(
    agent_label: Option<&'a str>,
    parent: Option<&'a str>,
    cdamux_id: Option<&'a str>,
    extra: &SpawnExtra<'a>,
) -> Vec<(&'static str, &'a str)> {
    let mut out = Vec::new();
    if let Some(label) = agent_label {
        out.push(("@cdamux_agent", label));
    }
    out.push(("@cdamux_role", resolve_role(agent_label, extra)));
    if let Some(id) = cdamux_id {
        out.push(("@cdamux_id", id));
    }
    if let Some(p) = parent {
        out.push(("@cdamux_parent", p));
    }
    if let Some(run) = extra.run.filter(|s| !s.is_empty()) {
        out.push(("@cdamux_run", run));
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
        let extra = SpawnExtra::default();
        assert_eq!(
            link_options(Some("claude"), Some("c-main"), Some("a-7f3k2"), &extra),
            vec![
                ("@cdamux_agent", "claude"),
                ("@cdamux_role", "agent"),
                ("@cdamux_id", "a-7f3k2"),
                ("@cdamux_parent", "c-main"),
            ]
        );
        assert_eq!(
            link_options(Some("shell"), None, None, &extra),
            vec![("@cdamux_agent", "shell"), ("@cdamux_role", "shell")]
        );
        let worker = SpawnExtra {
            role: Some("worker"),
            run: Some("r-abc12"),
            ..Default::default()
        };
        assert_eq!(
            link_options(Some("claude"), Some("c-orch"), Some("a-w1"), &worker),
            vec![
                ("@cdamux_agent", "claude"),
                ("@cdamux_role", "worker"),
                ("@cdamux_id", "a-w1"),
                ("@cdamux_parent", "c-orch"),
                ("@cdamux_run", "r-abc12"),
            ]
        );
    }

    #[test]
    fn parent_agent_id_resolves_registry_or_null() {
        use crate::paths::env_lock;
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        assert_eq!(parent_agent_id(None), None);
        assert_eq!(parent_agent_id(Some("c-missing")), None);
        session_registry::record(&session_registry::SessionRecord {
            name: "c-main".into(),
            cwd: "/tmp/p".into(),
            agent: "claude".into(),
            agent_session_id: None,
            cdamux_id: Some("a-parent".into()),
            parent: None,
            role: "agent".into(),
            layout: PaneLayout::Single,
            created_ts: 1,
            updated_ts: 1,
        })
        .unwrap();
        assert_eq!(parent_agent_id(Some("c-main")).as_deref(), Some("a-parent"));
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn restore_refresh_preserves_appended_fields() {
        use crate::paths::env_lock;
        use crate::state::state_get;
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());

        let id = "a-keep1";
        let extra = SpawnExtra::default();
        write_agent_state("c-old", Some("claude"), None, Some(id), &extra);
        state_append(
            StateKind::Agents,
            id,
            &serde_json::json!({"status": "idle", "resume_id": "conv-9"}),
        )
        .unwrap();

        refresh_agent_state_on_restore("c-new", Some("claude"), None, Some(id), &extra);

        let got = state_get(StateKind::Agents, id).unwrap().unwrap();
        assert_eq!(got["host"], "c-new");
        assert_eq!(got["status"], "idle");
        assert_eq!(got["resume_id"], "conv-9");
        assert_eq!(got["id"], id);
        assert_eq!(got["client"], "claude");
        // Baseline `run` was already present → not overwritten to null.
        assert!(got["run"].is_null());

        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn resolve_accepts_cursor_aliases() {
        let agents = AgentsConfig {
            default: "cursor".into(),
            cursor: Some(AgentEntry {
                command: "cursor-agent".into(),
                resume: None,
            }),
            opencode: None,
            claude: None,
            pi: None,
        };
        assert_eq!(resolve_agent_command(&agents, "agent").unwrap(), "cursor-agent");
        assert_eq!(resolve_agent_command(&agents, "cursor-agent").unwrap(), "cursor-agent");
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
        let extra = SpawnExtra::default();
        assert!(registry_record(
            "cdamux",
            p,
            Some("claude"),
            None,
            PaneLayout::Single,
            None,
            None,
            &extra,
        )
        .is_none());
        assert!(registry_record(
            "c-p",
            p,
            None,
            None,
            PaneLayout::Single,
            None,
            None,
            &extra
        )
        .is_none());
        let r = registry_record(
            "c-p",
            p,
            Some("shell"),
            Some("c-main"),
            PaneLayout::Vertical,
            None,
            Some("a-shell".into()),
            &extra,
        )
        .unwrap();
        assert_eq!(
            (
                r.agent.as_str(),
                r.role.as_str(),
                r.parent.as_deref(),
                r.layout,
                r.cdamux_id.as_deref()
            ),
            ("shell", "shell", Some("c-main"), PaneLayout::Vertical, Some("a-shell"))
        );
        assert_eq!(
            registry_record(
                "c-p",
                p,
                Some("cursor-agent"),
                None,
                PaneLayout::Single,
                Some("u".into()),
                Some("a-xyz12".into()),
                &extra,
            )
            .unwrap()
            .agent
            .as_str(),
            "cursor"
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
