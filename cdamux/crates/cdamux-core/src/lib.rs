//! Shared configuration and domain types for cdamux.

mod daemon;
mod diff;
mod notes;
mod send;
mod session;
mod spawn;
mod tailnet_api;
mod tmux;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use daemon::{
    daemon_list_sessions, grouped_for_list, handle_daemon_request, poll_sessions, socket_path,
    state_cache_path, write_state_cache,
};
pub use diff::{
    format_review_prompt, git_diff_unified, parse_unified_diff, DiffError, DiffHunk,
};
pub use session::{
    classify_session, enrich_sessions, group_sessions, EnrichedSession, GroupedSessions,
    SessionGroup, SessionState,
};
pub use tailnet_api::{
    decode_path_segment, dispatch_tailnet_request, resolve_tailnet_bind_ip, LiveTailnetBackend,
    TailnetApiBackend, TailnetResponse,
};
pub use notes::{ensure_note_file, note_refs_for_projects, resolve_note_path, NoteRef};
pub use send::{build_send_keys_argv, send_to_session, SendError, tmux_has_session};
pub use spawn::{
    configured_agent_ids, default_agent_name, resolve_agent_command, session_name_for_path,
    spawn_tmux_session, SpawnError, SpawnOutcome,
};
pub use tmux::{
    capture_pane_text, classify_pane_heuristics, list_tmux_sessions, parse_list_sessions,
    pane_signals_for_session, send_line_to_session, PaneSignals, TmuxError, TmuxSession,
};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("read config: {0}")]
    Read(String),
    #[error("parse config: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub projects: ProjectsConfig,
    #[serde(default)]
    pub sessions: SessionsConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub notes: NotesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            projects: ProjectsConfig::default(),
            sessions: SessionsConfig::default(),
            daemon: DaemonConfig::default(),
            agents: AgentsConfig::default(),
            ui: UiConfig::default(),
            notes: NotesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default = "default_discover")]
    pub discover: DiscoverMode,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default)]
    pub roots: Vec<ProjectRoot>,
}

fn default_discover() -> DiscoverMode {
    DiscoverMode::GitOnly
}

fn default_max_depth() -> u32 {
    6
}

impl Default for ProjectsConfig {
    fn default() -> Self {
        Self {
            discover: DiscoverMode::GitOnly,
            max_depth: 6,
            roots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRoot {
    pub path: PathBuf,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverMode {
    Flat,
    GitOnly,
    Depth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsConfig {
    #[serde(default)]
    pub attach_mode: AttachMode,
    #[serde(default)]
    pub group_by: GroupBy,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            attach_mode: AttachMode::Mirror,
            group_by: GroupBy::Project,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttachMode {
    #[default]
    Mirror,
    Exclusive,
}

/// Dynamic session list grouping (epilande ccmux parity target).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    #[default]
    Project,
    Cwd,
    Agent,
    GroupPath,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_poll")]
    pub poll_interval_seconds: u32,
    #[serde(default = "default_idle")]
    pub idle_seconds_for_needs_input: u32,
    #[serde(default)]
    pub listen_tailnet: bool,
    #[serde(default = "default_port")]
    pub tailnet_port: u16,
}

fn default_poll() -> u32 {
    2
}
fn default_idle() -> u32 {
    3
}
fn default_port() -> u16 {
    7480
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            poll_interval_seconds: 2,
            idle_seconds_for_needs_input: 3,
            listen_tailnet: false,
            tailnet_port: 7480,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default = "default_agent")]
    pub default: String,
    #[serde(default)]
    pub cursor: Option<AgentEntry>,
    #[serde(default)]
    pub opencode: Option<AgentEntry>,
    #[serde(default)]
    pub claude: Option<AgentEntry>,
    #[serde(default)]
    pub pi: Option<AgentEntry>,
}

fn default_agent() -> String {
    "cursor".to_string()
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            default: default_agent(),
            cursor: None,
            opencode: None,
            claude: None,
            pi: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    /// When empty, spawn uses the agent id as the executable name (ccmux parity).
    #[serde(default)]
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_ui_mode")]
    pub default_mode: UiMode,
}

fn default_ui_mode() -> UiMode {
    UiMode::Dashboard
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_mode: UiMode::Dashboard,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    Dashboard,
    Pick,
}

/// Markdown notes: per-project file or optional central directory (ADR-004 local only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesConfig {
    /// When set, notes live here as `{slug}.md` derived from the project path.
    #[serde(default)]
    pub central_root: Option<PathBuf>,
    /// Relative path under each project when `central_root` is unset.
    #[serde(default = "default_notes_relative")]
    pub relative_path: PathBuf,
    /// External editor command; empty uses `$EDITOR` then `vi`.
    #[serde(default)]
    pub editor: String,
}

fn default_notes_relative() -> PathBuf {
    PathBuf::from(".cdamux/notes.md")
}

impl Default for NotesConfig {
    fn default() -> Self {
        Self {
            central_root: None,
            relative_path: default_notes_relative(),
            editor: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub root_label: String,
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("cdamux").join("config.toml"))
}

pub fn load_config() -> Result<Config, ConfigError> {
    let path = config_path().ok_or_else(|| ConfigError::Read("no config path".into()))?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| ConfigError::Read(e.to_string()))?;
    toml::from_str(&raw).map_err(|e| ConfigError::Parse(e.to_string()))
}

/// Discover projects under configured roots (v0: flat + git_only).
pub fn discover_projects(cfg: &ProjectsConfig) -> Vec<Project> {
    let mut out = Vec::new();
    for root in &cfg.roots {
        let label = if root.label.is_empty() {
            root.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        } else {
            root.label.clone()
        };
        match cfg.discover {
            DiscoverMode::Flat => discover_flat(&root.path, &label, &mut out),
            DiscoverMode::GitOnly | DiscoverMode::Depth => {
                discover_git(&root.path, &label, cfg.max_depth, &mut out);
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

fn discover_flat(root: &Path, label: &str, out: &mut Vec<Project>) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        out.push(Project {
            name,
            path,
            root_label: label.to_string(),
        });
    }
}

fn discover_git(root: &Path, label: &str, max_depth: u32, out: &mut Vec<Project>) {
    walk_git(root, label, max_depth, 0, out);
}

fn walk_git(dir: &Path, label: &str, max_depth: u32, depth: u32, out: &mut Vec<Project>) {
    if depth > max_depth {
        return;
    }
    if dir.join(".git").exists() {
        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| dir.to_string_lossy().to_string());
        out.push(Project {
            name,
            path: dir.to_path_buf(),
            root_label: label.to_string(),
        });
        // Do not descend into submodules/repos inside a repo for v0.
        return;
    }
    if depth == max_depth {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        walk_git(&path, label, max_depth, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn config_parses_notes_section() {
        let raw = r#"
schema_version = 1
[notes]
central_root = "/tmp/notes"
relative_path = "notes.md"
editor = "nvim"
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.notes.central_root.as_deref(), Some(Path::new("/tmp/notes")));
        assert_eq!(cfg.notes.relative_path, PathBuf::from("notes.md"));
        assert_eq!(cfg.notes.editor, "nvim");
    }

    #[test]
    fn discover_git_finds_nested_repo() {
        let tmp = tempdir().unwrap();
        let org = tmp.path().join("org");
        let repo = org.join("my-app");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let cfg = ProjectsConfig {
            discover: DiscoverMode::GitOnly,
            max_depth: 4,
            roots: vec![ProjectRoot {
                path: tmp.path().to_path_buf(),
                label: "t".into(),
            }],
        };
        let found = discover_projects(&cfg);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("my-app"));
    }
}
