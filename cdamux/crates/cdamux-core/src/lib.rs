//! Shared configuration and domain types for cdamux.

mod agent_status;
mod agents_util;
mod board_index;
mod client_name;
mod daemon;
mod diff;
mod events;
mod foam;
mod home_migrate;
mod markdown;
mod notes;
mod issue_ref;
mod links;
mod msg;
mod orch;
mod orch_graph;
mod orch_graph_layout;
mod orch_status;
mod orch_steps;
mod paths;
mod project_tree;
mod restore;
mod say;
mod send;
mod session;
mod session_chrome;
mod session_links;
mod session_manage;
mod sidebar;
pub mod session_registry;
mod spawn;
mod state;
mod state_finalize;
mod state_gc;
mod subagent_ledger;
mod worktree;
mod worktree_clean;
mod worker;
mod tailnet_api;
mod tmux;
mod capture;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use daemon::{grouped_for_list, grouped_for_list_ex};
pub use events::{
    journal_stamp, last_turn_reply, parse_cursor, read_events, record_reply, EventFilter,
    EventsError, EVENTS_SCHEMA_VERSION, MAX_REPLY_BYTES,
};
pub use diff::{
    format_review_prompt, git_diff_unified, parse_unified_diff, DiffError, DiffHunk,
};
pub use project_tree::{
    build_project_session_tree, build_project_session_tree_ex, project_for_path, project_key,
    state_icon, ProjectSessionTree,
    ProjectTreeNode,
};
pub use session_links::{
    session_forest, tail_lines, tree_roots, ChildKind, ChildRow, PreviewSource, SessionForest,
};
pub use subagent_ledger::{session_subagents, subagent_ledger_path, transcript_tail};
pub use session_chrome::{session_agent_label, SessionChrome, AGENT_LABEL_MISSING};
pub use agent_status::{
    heuristic_applies, load_reported_status, may_overwrite, parse_reported_status,
    resolve_display_status,
    DEFAULT_WORKING_WARN_SECONDS,
};
pub use session::{
    classify_session, classify_session_ex, enrich_sessions, enrich_sessions_ex, group_sessions,
    parse_hook_session_state, EnrichedSession, GroupedSessions, SessionGroup, SessionState,
    ORCH_GLYPH,
};
pub use tailnet_api::{
    check_tailnet_token, decode_path_segment, dispatch_tailnet_request, resolve_tailnet_bind_ip,
    LiveTailnetBackend, TailnetApiBackend, TailnetIpError, TailnetResponse, TAILNET_TOKEN_HEADER,
};
pub use board_index::{
    board_index_file, refresh_board_index, BoardIndex, RefreshOptions, BOARD_INDEX_FILE,
};
pub use foam::{
    extract_foam_path_from_text, extract_wikilinks, find_foam_entry, foam_file_wikilinks,
    list_foam, normalize_foam_path, preview_foam_markdown, preview_foam_raw, resolve_foam_root,
    resolve_project_root, resolve_wikilink, FoamListing, FoamRow, FoamTree, WikiLink, FOAM_DIR,
    FOAM_LIST_CAP, FOAM_PREVIEW_MAX_BYTES, FOAM_PREVIEW_MAX_LINES,
};
pub use markdown::{render_markdown_terminal, truncate_lines};
pub use notes::{ensure_note_file, note_refs_for_projects, resolve_note_path, NoteRef};
pub use orch_graph_layout::{
    graph_node_order, layout_orch_graph, CellClass, GraphCell, GraphLayout, NodeBox, NodeState, BOX_H,
};
pub use orch_graph::{
    find_orch_graph_for_session, find_orch_graph_for_session_from_cwd, find_orch_graph_from_cwd,
    find_orch_run, find_orch_run_from_cwd, is_live_run_status, orch_graph_is_stale, orch_graph_header,
    format_orch_graph_panel, format_orch_graph_panel_ex, format_orch_graph_panel_with_history,
    list_live_orch_runs, list_orch_graph_runs, load_orch_graph, load_orch_graph_at, orch_graph_live_dir,
    orch_graph_path, orch_graph_runs_dir, run_repo_matches, OrchGraphError, OrchGraphFile, OrchGraphNode,
    OrchGraphRunSummary,
};
pub use issue_ref::{issue_from_branch_or_path, parse_issue_number};
pub use links::{
    agent_cwd, clear_links, parse_link_ref, refresh_links, set_link_override, AgentLinks, LinkRef,
    LINK_TTL_SECS,
};
pub use orch_status::{
    find_orch_run_status_from_cwd, load_orch_run_status, orch_agents_dir, project_has_orch_artifacts,
    OrchRunStatusFile, OrchWorkerStatus, PhaseMark,
};
pub use orch_steps::{step_worker_id, worker_steps, Step};
pub use agents_util::{
    command_on_path, preferred_spawn_agent, preferred_spawn_agent_index,
};
pub use client_name::{is_product_client, normalize_client_name};
pub use home_migrate::{
    apply_home_migration, legacy_home, plan_home_migration, HomeMigration,
};
pub use paths::{cdamux_home, explicit_cdamux_home, cdamux_state_dir, env_lock};
pub use orch::{
    agent_scope_is_global, find_live_global_orchestrator, global_orch_cwd, orch_boot_prompt,
    orch_boot_prompt_for, orch_boot_prompt_global, spawn_orchestrator, OrchError, OrchNewRequest,
    OrchNewResult, ORCHESTRATOR_SKILL, ORCH_SCOPE_GLOBAL,
};
pub use orch::{list_orchestrators, orch_scope_of_agent, OrchEntry, ORCH_SCOPE_PROJECT};
pub use capture::{capture_session, CaptureError, PaneCapture, CAPTURE_MAX_LINES};
pub use state::{
    agent_state_path, generate_agent_id, generate_run_id, initial_agent_value, state_append,
    state_append_sticky, state_get, state_list, state_set, state_update, valid_state_id,
    StateKind,
};
pub use state_finalize::{
    agent_subtree, apply_dismiss, finalize_session_host, kill_agent_permanent, kill_permanent, orphan_agent_eligible, orphan_run_eligible,
    plan_dismiss_agent, plan_dismiss_run, sweep_orphans, DismissPlan, DismissReject, FinalizeReport,
};
pub use state_gc::{
    agent_eligible, agent_has_no_host, archive_agent, archive_run, is_gc_agent_status,
    is_gc_run_status, is_resumable_dormant, run_eligible, run_gc, run_gc_agents_only, GcReport,
    DEFAULT_RETENTION_DAYS,
};
pub use restore::{
    dormant, is_busy_state, is_done_record, last_global_orch_record, parents_first,
    pick_global_orch_record, reap_dead_agent_panes, alert_long_blocked, tick_long_blocked, BLOCKED_ALERT_SECS, restart_plan, restore_one, restore_subtree,
    restore_targets, resume_command, RestoreError, RestoreOutcome, DEAD_AGENT_PANE_SECS,
};
pub use msg::{
    agent_parent_id, append_message, ask_answered, enable_background_waker, find_message,
    format_unread_digest, generate_msg_id, has_pending_ask, inbox_badges, msg_send,
    read_inbox_all, read_messages, read_messages_max, reply_to_ask, resolve_agent_id, run_waker, unread_count,
    valid_kind, wake_once, wake_plan, wake_quiet_secs, wake_text, Delivery, Message, MsgError,
    SendOutcome, WakePlan, DEFAULT_NUDGE_QUIET_SECS, DEFAULT_WAKE_QUIET_SECS, MAX_MSG_BYTES,
};
pub use say::{answer_ask, say, say_drain, AnswerResult, SayOutcome, SayResult};
pub use sidebar::{
    client_session, default_client, find_sidebar, focus_main_pane, follow_session, spawn_sidebar,
    SidebarSide, DEFAULT_SIDEBAR_WIDTH,
};
pub use send::{send_input, send_to_session, SendError, InputOutcome, SEND_KEYS, tmux_has_session};
pub use session_manage::{
    current_tmux_session, ensure_home_session, ensure_nav_session, home_session_new_argv,
    is_home_session, is_nav_session, kill_tmux_session, leave_nav_to_session,
    mark_session_cdamux_return, nav_return_or_home, neutralize_detach_on_destroy,
    rename_tmux_session,
    should_ensure_home_before_kill, switch_client_to_session, toggle_nav_client,
    unique_session_name,
};
pub use spawn::{
    configured_agent_ids, default_agent_name, default_shell_command, resolve_agent_command,
    session_name_for_path, spawn_tmux_session_ex, spawn_tmux_session_with_layout, PaneLayout,
    SpawnError, SpawnExtra, SpawnOutcome,
};
pub use worker::{
    compose_worker_first_prompt, count_live_workers, ensure_orch_run, kill_worker, list_workers,
    peek_worker, KilledAgent,
    shell_single_quote, spawn_worker, with_first_prompt, worker_ls_line, worker_preamble,
    WorkerError, WorkerRow,
    WorkerSpawnRequest, WorkerSpawnResult,
};
pub use worktree::{
    current_branch, git_toplevel, list_local_branches, list_worktrees, resolve_worktree_dest,
    worktree_add_new_branch, WorktreeEntry, WorktreeError,
};
pub use worktree_clean::{
    check_free_space, checkout_bytes, clean_verdict, clean_worktrees, format_bytes, gh_pr_state,
    registry_paths, CleanItem, PrState,
    Verdict, WorktreeSession, DEFAULT_DISPOSABLE, DEFAULT_MIN_FREE_GIB,
};
pub use tmux::{
    capture_pane_text, classify_pane_heuristics, is_hidden_fleet_session, list_panes_all,
    list_tmux_sessions, list_tmux_sessions_strict, parse_list_panes_all, parse_list_sessions, send_line_to_session,
    session_owner, tmux_user_option,
    SessionPaneInfo, HOME_SESSION_NAME, NAV_SESSION_NAME, TmuxError, TmuxSession,
};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("read config: {0}")]
    Read(String),
    #[error("parse config: {0}")]
    Parse(String),
    #[error("write config: {0}")]
    Write(String),
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
    #[serde(default)]
    pub worktrees: WorktreesConfig,
    /// Retention for `cdamux state gc` (ADR-011 / PRD-239).
    #[serde(default)]
    pub state: StateConfig,
    /// Machine-wide orchestrator policy (ADR-015 / PRD-357).
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,
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
            worktrees: WorktreesConfig::default(),
            state: StateConfig::default(),
            orchestrator: OrchestratorConfig::default(),
        }
    }
}

/// `[orchestrator]` — machine-wide caps for `cdamux worker spawn` (ADR-015).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Cap on live workers across all projects (`0` = no cap). Default 6.
    #[serde(default = "default_max_workers")]
    pub max_workers: u32,
}

fn default_max_workers() -> u32 {
    6
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_workers: default_max_workers(),
        }
    }
}

/// `$CDAMUX_HOME` retention policy (`cdamux state gc`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    /// Days before done/error agents and finished runs (no host) are archived.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 {
    DEFAULT_RETENTION_DAYS
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
        }
    }
}

/// Optional parent directory for spawn-time `git worktree add` (ADR-003: not orch-auto),
/// plus build-cache cleanup and the spawn disk guard (ADR-017).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreesConfig {
    #[serde(default)]
    pub root: Option<PathBuf>,
    /// Git-ignored dirs `cdamux worker clean` may delete in finished worktrees.
    #[serde(
        default = "default_disposable",
        deserialize_with = "deserialize_disposable"
    )]
    pub disposable: Vec<String>,
    /// `worker spawn` refuses below this free space on the worktree filesystem (`0` = off).
    #[serde(default = "default_min_free_gib")]
    pub min_free_gib: u64,
}

fn default_disposable() -> Vec<String> {
    worktree_clean::DEFAULT_DISPOSABLE
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_min_free_gib() -> u64 {
    worktree_clean::DEFAULT_MIN_FREE_GIB
}

fn deserialize_disposable<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Vec::<String>::deserialize(d)?;
    for e in &v {
        worktree_clean::validate_disposable(e).map_err(serde::de::Error::custom)?;
    }
    Ok(v)
}

impl Default for WorktreesConfig {
    fn default() -> Self {
        Self {
            root: None,
            disposable: default_disposable(),
            min_free_gib: default_min_free_gib(),
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
    /// Quiet `working` pane (seconds) before Fleet shows `working ⚠` (ADR-011 §3).
    #[serde(default = "default_working_warn")]
    pub working_warn_seconds: u32,
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
fn default_working_warn() -> u32 {
    agent_status::DEFAULT_WORKING_WARN_SECONDS
}
fn default_port() -> u16 {
    7480
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            poll_interval_seconds: 2,
            idle_seconds_for_needs_input: 3,
            working_warn_seconds: default_working_warn(),
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
    /// Resume command template with `{id}` (the agent conversation id), used by
    /// `cdamux restore`. Unset → built-in default per agent; `""` → never resume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_ui_mode")]
    pub default_mode: UiMode,
    /// Max lines for dashboard pane preview (tmux capture-pane).
    #[serde(default = "default_preview_max_lines")]
    pub preview_max_lines: u32,
}

fn default_ui_mode() -> UiMode {
    UiMode::Dashboard
}

fn default_preview_max_lines() -> u32 {
    80
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_mode: UiMode::Dashboard,
            preview_max_lines: default_preview_max_lines(),
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
    if let Some(p) = std::env::var_os("CDAMUX_CONFIG").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("cdamux").join("config.toml"))
}

pub fn load_config() -> Result<Config, ConfigError> {
    let path = config_path().ok_or_else(|| ConfigError::Read("no config path".into()))?;
    load_config_from(&path)
}

/// Load config from an explicit path (missing file → defaults).
pub fn load_config_from(path: &std::path::Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| ConfigError::Read(e.to_string()))?;
    toml::from_str(&raw).map_err(|e| ConfigError::Parse(e.to_string()))
}

/// Persist config to the XDG `config.toml` (creates parent dir if needed).
pub fn save_config(cfg: &Config) -> Result<(), ConfigError> {
    let path = config_path().ok_or_else(|| ConfigError::Write("no config path".into()))?;
    save_config_to(cfg, &path)
}

/// Persist config to an explicit path (creates parent dir if needed).
pub fn save_config_to(cfg: &Config, path: &std::path::Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConfigError::Write(e.to_string()))?;
    }
    let raw = toml::to_string_pretty(cfg).map_err(|e| ConfigError::Write(e.to_string()))?;
    std::fs::write(path, raw).map_err(|e| ConfigError::Write(e.to_string()))?;
    Ok(())
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

/// Resolve `worker spawn --repo <path|project>` to a git toplevel.
///
/// Existing directory → `git_toplevel` (or [`ResolveSpawnRepoError::NotGit`]).
/// Otherwise match `Project.name` from [`discover_projects`]: one hit → its path;
/// zero → unknown; several → ambiguous with `root_label/name` candidates.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolveSpawnRepoError {
    #[error("path is not inside a git repository")]
    NotGit,
    #[error("unknown project '{0}'")]
    Unknown(String),
    #[error("ambiguous project '{name}': {candidates}")]
    Ambiguous { name: String, candidates: String },
}

pub fn resolve_spawn_repo(
    cfg: &ProjectsConfig,
    value: &str,
) -> Result<PathBuf, ResolveSpawnRepoError> {
    let path = Path::new(value);
    if path.is_dir() {
        return git_toplevel(path).ok_or(ResolveSpawnRepoError::NotGit);
    }
    let found = discover_projects(cfg);
    let matches: Vec<&Project> = found.iter().filter(|p| p.name == value).collect();
    match matches.as_slice() {
        [] => Err(ResolveSpawnRepoError::Unknown(value.to_string())),
        [one] => Ok(one.path.clone()),
        many => Err(ResolveSpawnRepoError::Ambiguous {
            name: value.to_string(),
            candidates: many
                .iter()
                .map(|p| format!("{}/{}", p.root_label, p.name))
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
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
    fn config_parses_state_retention_default_fourteen() {
        let cfg: Config = toml::from_str("schema_version = 1\n").unwrap();
        assert_eq!(cfg.state.retention_days, 14);

        let raw = r#"
schema_version = 1
[state]
retention_days = 7
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.state.retention_days, 7);
    }

    #[test]
    fn config_parses_orchestrator_max_workers_default_six() {
        let cfg: Config = toml::from_str("schema_version = 1\n").unwrap();
        assert_eq!(cfg.orchestrator.max_workers, 6);

        let raw = r#"
schema_version = 1
[orchestrator]
max_workers = 0
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.orchestrator.max_workers, 0);
    }

    #[test]
    fn config_parses_worktrees_disposable_and_guard() {
        let cfg: Config = toml::from_str("schema_version = 1\n").unwrap();
        assert_eq!(cfg.worktrees.disposable, vec!["target", "node_modules", ".nuxt", ".output"]);
        assert_eq!(cfg.worktrees.min_free_gib, 40);

        let raw = r#"
schema_version = 1
[worktrees]
disposable = ["cdamux/target", "dist"]
min_free_gib = 0
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.worktrees.disposable, vec!["cdamux/target", "dist"]);
        assert_eq!(cfg.worktrees.min_free_gib, 0);

        for bad in ["/tmp", "../up", "a/../b", ""] {
            let raw = format!("schema_version = 1\n[worktrees]\ndisposable = [\"{bad}\"]\n");
            assert!(toml::from_str::<Config>(&raw).is_err(), "{bad:?}");
        }
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

    fn init_git_repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        assert!(std::process::Command::new("git")
            .args(["-C", dir.to_str().unwrap(), "init", "-b", "main"])
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn resolve_spawn_repo_path_and_name() {
        let tmp = tempdir().unwrap();
        let root_a = tmp.path().join("a");
        let root_b = tmp.path().join("b");
        let repo_a = root_a.join("app");
        let repo_b = root_b.join("app");
        let alone = root_a.join("solo");
        init_git_repo(&repo_a);
        init_git_repo(&repo_b);
        init_git_repo(&alone);
        let not_git = tmp.path().join("plain");
        fs::create_dir_all(&not_git).unwrap();

        let cfg = ProjectsConfig {
            discover: DiscoverMode::GitOnly,
            max_depth: 4,
            roots: vec![
                ProjectRoot {
                    path: root_a.clone(),
                    label: "alpha".into(),
                },
                ProjectRoot {
                    path: root_b.clone(),
                    label: "beta".into(),
                },
            ],
        };

        assert_eq!(
            resolve_spawn_repo(&cfg, alone.to_str().unwrap()).unwrap(),
            git_toplevel(&alone).unwrap()
        );
        assert_eq!(
            resolve_spawn_repo(&cfg, "solo").unwrap(),
            alone
        );
        assert_eq!(
            resolve_spawn_repo(&cfg, not_git.to_str().unwrap()),
            Err(ResolveSpawnRepoError::NotGit)
        );
        assert_eq!(
            resolve_spawn_repo(&cfg, "missing"),
            Err(ResolveSpawnRepoError::Unknown("missing".into()))
        );
        match resolve_spawn_repo(&cfg, "app") {
            Err(ResolveSpawnRepoError::Ambiguous { name, candidates }) => {
                assert_eq!(name, "app");
                assert!(candidates.contains("alpha/app"));
                assert!(candidates.contains("beta/app"));
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }
}
