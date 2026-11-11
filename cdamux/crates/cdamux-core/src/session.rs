//! Session list enrichment and grouping.

use crate::agent_status::{
    load_reported_status, parse_reported_status, resolve_display_status,
    DEFAULT_WORKING_WARN_SECONDS,
};
use crate::session_chrome::{resolve_agent_label, AGENT_LABEL_MISSING};
use crate::project_tree::project_for_path;
use crate::tmux::{list_panes_all, SessionPaneInfo};
use crate::{discover_projects, GroupBy, Project, ProjectsConfig, TmuxSession};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Agent / session display status (ADR-011 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Working,
    NeedsInput,
    /// Waiting on the orchestrator or a dependency (not the user).
    Blocked,
    /// Hook / worker reported finished.
    Done,
    /// Hook / worker reported failure.
    Error,
    /// No live host; resumable (ADR-009). Reader-computed when host is missing.
    Dormant,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::Idle
    }
}

fn default_agent_label() -> String {
    AGENT_LABEL_MISSING.into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnrichedSession {
    pub name: String,
    pub path: String,
    pub group_key: String,
    pub state: SessionState,
    /// Reported `working` with a quiet pane past the warn threshold (display only).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub working_stale: bool,
    /// From `@cdamux_agent` / `@ccmux_agent`; [`AGENT_LABEL_MISSING`] when unset.
    #[serde(default = "default_agent_label")]
    pub agent: String,
    /// Parsed from `feat/<n>-*` branch or `.orch/worktrees/<n>-*` path when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u32>,
    /// Session links (`@cdamux_parent` / `@cdamux_run` / `@cdamux_role`), see [`TmuxSession`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGroup {
    pub key: String,
    pub sessions: Vec<EnrichedSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedSessions {
    pub group_by: GroupBy,
    pub groups: Vec<SessionGroup>,
}

/// Map `@cdamux_state` / reported status strings → [`SessionState`].
pub fn parse_hook_session_state(raw: &str) -> Option<SessionState> {
    parse_reported_status(raw)
}

/// Prefer `CDAMUX_HOME` agent status, then transitional `@cdamux_state`, else pane heuristics.
pub fn classify_session(
    session: &TmuxSession,
    info: &SessionPaneInfo,
    idle_seconds_for_needs_input: u32,
) -> SessionState {
    classify_session_ex(
        session,
        info,
        idle_seconds_for_needs_input,
        DEFAULT_WORKING_WARN_SECONDS,
    )
    .0
}

/// Like [`classify_session`], also returning the `working ⚠` display flag.
pub fn classify_session_ex(
    session: &TmuxSession,
    info: &SessionPaneInfo,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
) -> (SessionState, bool) {
    let reported = info
        .cdamux_id
        .as_deref()
        .and_then(load_reported_status)
        .or_else(|| info.hook_state.as_deref().and_then(parse_hook_session_state));

    let heuristic = if reported.is_some() {
        // Stickiness / reported win: skip capture-pane when we already have a signal.
        SessionState::Idle
    } else if info.idle_seconds.is_none() {
        SessionState::Idle
    } else {
        match crate::tmux::pane_tail(&session.name) {
            Some(tail) => crate::tmux::classify_pane_heuristics(
                info.idle_seconds,
                &tail,
                idle_seconds_for_needs_input,
            ),
            None => SessionState::Idle,
        }
    };

    resolve_display_status(
        true,
        reported,
        heuristic,
        info.idle_seconds,
        working_warn_seconds,
    )
}

pub fn enrich_sessions(
    raw: &[TmuxSession],
    group_by: GroupBy,
    projects_cfg: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> Vec<EnrichedSession> {
    enrich_sessions_ex(
        raw,
        group_by,
        projects_cfg,
        idle_seconds_for_needs_input,
        DEFAULT_WORKING_WARN_SECONDS,
    )
}

pub fn enrich_sessions_ex(
    raw: &[TmuxSession],
    group_by: GroupBy,
    projects_cfg: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
) -> Vec<EnrichedSession> {
    // Only GroupPath needs the project list; the tree builder discovers for Project itself.
    let projects = if group_by == GroupBy::GroupPath {
        discover_projects(projects_cfg)
    } else {
        Vec::new()
    };
    let panes = list_panes_all();
    raw.iter()
        .map(|s| {
            let info = panes.get(&s.name).cloned().unwrap_or_default();
            let key = group_key_for(s, group_by, &projects);
            let branch = crate::session_chrome::branch_for_cwd(&s.path);
            let issue = crate::issue_ref::issue_from_branch_or_path(&branch, &s.path);
            let (state, working_stale) =
                classify_session_ex(s, &info, idle_seconds_for_needs_input, working_warn_seconds);
            EnrichedSession {
                name: s.name.clone(),
                path: s.path.clone(),
                group_key: key,
                state,
                working_stale,
                agent: resolve_agent_label(
                    info.cdamux_agent.as_deref(),
                    info.ccmux_agent.as_deref(),
                ),
                issue,
                parent: s.parent.clone(),
                run: s.run.clone(),
                role: s.role.clone(),
            }
        })
        .collect()
}

pub fn group_sessions(sessions: Vec<EnrichedSession>, group_by: GroupBy) -> GroupedSessions {
    let mut map: BTreeMap<String, Vec<EnrichedSession>> = BTreeMap::new();
    for s in sessions {
        let key = if group_by == GroupBy::None {
            String::new()
        } else {
            s.group_key.clone()
        };
        map.entry(key).or_default().push(s);
    }

    let groups = map
        .into_iter()
        .map(|(key, sessions)| SessionGroup {
            key: if key.is_empty() {
                "(all)".to_string()
            } else {
                key
            },
            sessions,
        })
        .collect();

    GroupedSessions {
        group_by,
        groups,
    }
}

#[cfg(test)]
mod hook_tests {
    use super::*;

    #[test]
    fn hook_state_parsing() {
        assert_eq!(
            parse_hook_session_state("running"),
            Some(SessionState::Working)
        );
        assert_eq!(
            parse_hook_session_state("waiting"),
            Some(SessionState::NeedsInput)
        );
        assert_eq!(parse_hook_session_state("blocked"), Some(SessionState::Blocked));
        assert_eq!(parse_hook_session_state("done"), Some(SessionState::Done));
        assert_eq!(parse_hook_session_state("error"), Some(SessionState::Error));
        assert_eq!(parse_hook_session_state("nope"), None);
    }

    #[test]
    fn agent_file_status_beats_hook_state() {
        use crate::paths::env_lock;
        use crate::state::{state_append, state_set, StateKind};
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let id = "a-fleet1";
        state_set(
            StateKind::Agents,
            id,
            &serde_json::json!({"id": id, "client": "claude"}),
        )
        .unwrap();
        state_append(
            StateKind::Agents,
            id,
            &serde_json::json!({"status": "working"}),
        )
        .unwrap();
        let session = TmuxSession {
            name: "s1".into(),
            ..Default::default()
        };
        let info = SessionPaneInfo {
            idle_seconds: Some(9999),
            hook_state: Some("idle".into()),
            cdamux_id: Some(id.into()),
            ..Default::default()
        };
        let (state, stale) = classify_session_ex(&session, &info, 3, 600);
        assert_eq!(state, SessionState::Working);
        assert!(stale);
        std::env::remove_var("CDAMUX_HOME");
    }
}

fn group_key_for(session: &TmuxSession, group_by: GroupBy, projects: &[Project]) -> String {
    match group_by {
        GroupBy::None => String::new(),
        GroupBy::Cwd => {
            if session.path.is_empty() {
                "(no cwd)".to_string()
            } else {
                session.path.clone()
            }
        }
        GroupBy::Project => project_name_from_path(&session.path),
        GroupBy::GroupPath => group_path_key(&session.path, projects),
        GroupBy::Agent => agent_key_from_session_name(&session.name),
    }
}

fn project_name_from_path(path: &str) -> String {
    if path.is_empty() {
        return "(no cwd)".to_string();
    }
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn group_path_key(path: &str, projects: &[Project]) -> String {
    if path.is_empty() {
        return "(no cwd)".to_string();
    }
    let p = Path::new(path);
    let best = project_for_path(path, projects);
    match best {
        Some(proj) => {
            let rel = p.strip_prefix(&proj.path).unwrap_or(p);
            let rel = rel.to_string_lossy();
            if rel.is_empty() || rel == "." {
                format!("{}/{}", proj.root_label, proj.name)
            } else {
                format!("{}/{}", proj.root_label, rel)
            }
        }
        None => project_name_from_path(path),
    }
}

/// Stub until pane/process inspection exists.
fn agent_key_from_session_name(name: &str) -> String {
    // Optional future pattern: c-<agent>-<project>
    if let Some(rest) = name.strip_prefix("c-") {
        if let Some((agent, _)) = rest.split_once('-') {
            if !agent.is_empty() {
                return agent.to_string();
            }
        }
    }
    "default".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiscoverMode, ProjectRoot};

    #[test]
    fn groups_by_project_basename() {
        let raw = vec![
            TmuxSession {
                name: "c-a".into(),
                path: "/hdd/dev/org/a".into(),
                ..Default::default()
            },
            TmuxSession {
                name: "c-b".into(),
                path: "/hdd/dev/org/b".into(),
                ..Default::default()
            },
        ];
        let cfg = ProjectsConfig::default();
        let enriched = enrich_sessions(&raw, GroupBy::Project, &cfg, 3);
        let grouped = group_sessions(enriched, GroupBy::Project);
        assert_eq!(grouped.groups.len(), 2);
    }

    #[test]
    fn group_path_uses_root_label() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("org").join("my-app");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let raw = vec![TmuxSession {
            name: "c-x".into(),
            path: repo.to_string_lossy().into(),
            ..Default::default()
        }];
        let cfg = ProjectsConfig {
            discover: DiscoverMode::GitOnly,
            max_depth: 4,
            roots: vec![ProjectRoot {
                path: tmp.path().to_path_buf(),
                label: "dev".into(),
            }],
        };
        let enriched = enrich_sessions(&raw, GroupBy::GroupPath, &cfg, 3);
        assert!(enriched[0].group_key.contains("dev/"));
    }
}
