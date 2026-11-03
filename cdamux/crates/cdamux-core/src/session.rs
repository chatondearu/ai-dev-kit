//! Session list enrichment and grouping.

use crate::session_chrome::{resolve_agent_label, AGENT_LABEL_MISSING};
use crate::project_tree::project_for_path;
use crate::tmux::{list_panes_all, SessionPaneInfo};
use crate::{discover_projects, GroupBy, Project, ProjectsConfig, TmuxSession};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Daemon-side session state (v0: stub classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Active,
    NeedsInput,
    /// Hook `@cdamux_state=done` (agent finished).
    Done,
    /// Hook `@cdamux_state=error` (agent failed / needs human).
    Error,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::Idle
    }
}

fn default_agent_label() -> String {
    AGENT_LABEL_MISSING.into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedSession {
    pub name: String,
    pub path: String,
    pub group_key: String,
    pub state: SessionState,
    /// From `@cdamux_agent` / `@ccmux_agent`; [`AGENT_LABEL_MISSING`] when unset.
    #[serde(default = "default_agent_label")]
    pub agent: String,
    /// Parsed from `feat/<n>-*` branch or `.orch/worktrees/<n>-*` path when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u32>,
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

/// Map `@cdamux_state` / `@cdamux_state` hook values → [`SessionState`].
pub fn parse_hook_session_state(raw: &str) -> Option<SessionState> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "running" | "active" => Some(SessionState::Active),
        "waiting" | "needs_input" | "needs-input" => Some(SessionState::NeedsInput),
        "done" => Some(SessionState::Done),
        "idle" => Some(SessionState::Idle),
        "error" | "failed" | "blocked" => Some(SessionState::Error),
        _ => None,
    }
}

/// Hook `@cdamux_state` wins; otherwise pane heuristics (one `capture-pane` only when idle is known).
pub fn classify_session(
    session: &TmuxSession,
    info: &SessionPaneInfo,
    idle_seconds_for_needs_input: u32,
) -> SessionState {
    if let Some(state) = info.hook_state.as_deref().and_then(parse_hook_session_state) {
        return state;
    }
    // Unknown idle classifies Idle whatever the tail says: skip the capture spawn.
    if info.idle_seconds.is_none() {
        return SessionState::Idle;
    }
    match crate::tmux::pane_tail(&session.name) {
        Some(tail) => crate::tmux::classify_pane_heuristics(
            info.idle_seconds,
            &tail,
            idle_seconds_for_needs_input,
        ),
        None => SessionState::Idle,
    }
}

pub fn enrich_sessions(
    raw: &[TmuxSession],
    group_by: GroupBy,
    projects_cfg: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
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
            EnrichedSession {
                name: s.name.clone(),
                path: s.path.clone(),
                group_key: key,
                state: classify_session(s, &info, idle_seconds_for_needs_input),
                agent: resolve_agent_label(
                    info.cdamux_agent.as_deref(),
                    info.ccmux_agent.as_deref(),
                ),
                issue,
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
            Some(SessionState::Active)
        );
        assert_eq!(
            parse_hook_session_state("waiting"),
            Some(SessionState::NeedsInput)
        );
        assert_eq!(parse_hook_session_state("done"), Some(SessionState::Done));
        assert_eq!(parse_hook_session_state("error"), Some(SessionState::Error));
        assert_eq!(parse_hook_session_state("nope"), None);
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
            },
            TmuxSession {
                name: "c-b".into(),
                path: "/hdd/dev/org/b".into(),
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
