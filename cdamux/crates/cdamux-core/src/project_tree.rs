//! Join discovered projects with tmux sessions for the Fleet tree UI.

use crate::orch_status::project_has_orch_artifacts;
use crate::session::{EnrichedSession, SessionState};
use crate::session_links::tree_roots;
use crate::{
    discover_projects, group_sessions, list_tmux_sessions, GroupBy, GroupedSessions, Project,
    ProjectsConfig,
};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProjectSessionTree {
    pub nodes: Vec<ProjectTreeNode>,
    pub unassigned: Vec<EnrichedSession>,
}

#[derive(Debug, Clone)]
pub struct ProjectTreeNode {
    pub project: Project,
    pub sessions: Vec<EnrichedSession>,
}

/// Longest matching discovered project for a session cwd.
pub fn project_for_path<'a>(session_path: &str, projects: &'a [Project]) -> Option<&'a Project> {
    if session_path.is_empty() {
        return None;
    }
    let p = Path::new(session_path);
    let mut best: Option<&Project> = None;
    for proj in projects {
        if p.starts_with(&proj.path) {
            if best.is_none() || proj.path.as_os_str().len() > best.unwrap().path.as_os_str().len() {
                best = Some(proj);
            }
        }
    }
    best
}

pub fn project_key(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// `keep`: cwds of restore records, so a project with only dormant sessions keeps its node.
pub fn build_project_session_tree(
    projects_cfg: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
    group_by: GroupBy,
    keep: &[&Path],
) -> ProjectSessionTree {
    build_project_session_tree_ex(
        projects_cfg,
        idle_seconds_for_needs_input,
        crate::agent_status::DEFAULT_WORKING_WARN_SECONDS,
        group_by,
        keep,
    )
}

/// Like [`build_project_session_tree`] with a configurable `working ⚠` threshold.
pub fn build_project_session_tree_ex(
    projects_cfg: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
    group_by: GroupBy,
    keep: &[&Path],
) -> ProjectSessionTree {
    let raw = list_tmux_sessions().unwrap_or_default();
    let enriched = crate::session::enrich_sessions_ex(
        &raw,
        group_by,
        projects_cfg,
        idle_seconds_for_needs_input,
        working_warn_seconds,
    );

    if group_by != GroupBy::Project {
        return tree_from_grouped(group_sessions(enriched, group_by));
    }

    let projects = discover_projects(projects_cfg);
    let kept: Vec<String> = keep
        .iter()
        .filter_map(|cwd| project_for_path(&cwd.to_string_lossy(), &projects))
        .map(|p| project_key(&p.path))
        .collect();
    let (mut by_project, mut unassigned) = assign_to_projects(enriched, &projects);

    let nodes = projects
        .into_iter()
        .filter_map(|project| {
            let key = project_key(&project.path);
            let mut sessions = by_project.remove(&key).unwrap_or_default();
            if sessions.is_empty() && !kept.contains(&key) && !project_has_orch_artifacts(&project.path) {
                return None;
            }
            sessions.sort_by(|a, b| a.name.cmp(&b.name));
            Some(ProjectTreeNode { project, sessions })
        })
        .collect();

    unassigned.sort_by(|a, b| a.name.cmp(&b.name));

    ProjectSessionTree { nodes, unassigned }
}

/// Sessions per project key (every project present) and the unassigned rest. A session
/// goes where its tree's root sits, so workers in `$CDAMUX_HOME/worktrees` stay under
/// their orchestrator's project instead of landing outside every project.
fn assign_to_projects(
    enriched: Vec<EnrichedSession>,
    projects: &[Project],
) -> (BTreeMap<String, Vec<EnrichedSession>>, Vec<EnrichedSession>) {
    let anchors: Vec<String> = tree_roots(&enriched).into_iter().map(|r| enriched[r].path.clone()).collect();
    let mut by_project: BTreeMap<String, Vec<EnrichedSession>> =
        projects.iter().map(|p| (project_key(&p.path), Vec::new())).collect();
    let mut unassigned = Vec::new();
    for (session, anchor) in enriched.into_iter().zip(anchors) {
        match project_for_path(&anchor, projects) {
            Some(proj) => by_project.entry(project_key(&proj.path)).or_default().push(session),
            None => unassigned.push(session),
        }
    }
    (by_project, unassigned)
}

fn tree_from_grouped(grouped: GroupedSessions) -> ProjectSessionTree {
    let nodes = grouped
        .groups
        .into_iter()
        .filter_map(|g| {
            let mut sessions = g.sessions;
            if sessions.is_empty() {
                return None;
            }
            sessions.sort_by(|a, b| a.name.cmp(&b.name));
            let path = sessions
                .first()
                .map(|s| Path::new(&s.path))
                .unwrap_or_else(|| Path::new(""));
            Some(ProjectTreeNode {
                project: Project {
                    name: g.key,
                    path: path.to_path_buf(),
                    root_label: format!("{:?}", grouped.group_by),
                },
                sessions,
            })
        })
        .collect();
    ProjectSessionTree {
        nodes,
        unassigned: Vec::new(),
    }
}

pub fn state_icon(state: SessionState) -> char {
    match state {
        SessionState::Idle => '○',
        SessionState::Working => '●',
        SessionState::NeedsInput => '◆',
        SessionState::Blocked => '⊘',
        SessionState::Done => '✓',
        SessionState::Error => '✗',
        SessionState::Dormant => '◌',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiscoverMode, ProjectRoot};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn longest_prefix_wins() {
        let tmp = tempdir().unwrap();
        let outer = tmp.path().join("org");
        let inner = outer.join("repo");
        fs::create_dir_all(inner.join(".git")).unwrap();
        let projects = vec![
            Project {
                name: "org".into(),
                path: outer.clone(),
                root_label: "dev".into(),
            },
            Project {
                name: "repo".into(),
                path: inner.clone(),
                root_label: "dev".into(),
            },
        ];
        let hit = project_for_path(&inner.to_string_lossy(), &projects).unwrap();
        assert_eq!(hit.name, "repo");
    }

    #[test]
    fn worker_outside_every_project_follows_its_orchestrator() {
        let projects = vec![Project { name: "a".into(), path: PathBuf::from("/src/a"), root_label: "dev".into() }];
        let s = |name: &str, path: &str, parent: Option<&str>| EnrichedSession {
            name: name.into(),
            path: path.into(),
            parent: parent.map(Into::into),
            ..Default::default()
        };
        let (by_project, unassigned) = assign_to_projects(
            vec![s("orch", "/src/a", None), s("w", "/home/.cdamux/worktrees/r/w", Some("orch")), s("loner", "/elsewhere", None)],
            &projects,
        );
        let names: Vec<&str> = by_project["/src/a"].iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["orch", "w"]);
        assert_eq!(unassigned.len(), 1);
    }

    #[test]
    fn unassigned_when_no_discover_match() {
        let projects = vec![Project {
            name: "a".into(),
            path: PathBuf::from("/tmp/a"),
            root_label: "dev".into(),
        }];
        assert!(project_for_path("/other/path", &projects).is_none());
    }

    #[test]
    fn tree_from_grouped_agent_buckets() {
        use crate::session::{EnrichedSession, SessionState};
        let grouped = group_sessions(
            vec![
                EnrichedSession {
                    name: "c-a".into(),
                    path: "/tmp/a".into(),
                    group_key: "cursor".into(),
                    state: SessionState::Idle,
                    agent: "cursor".into(),
                    issue: None,
                    ..Default::default()
                },
                EnrichedSession {
                    name: "o-b".into(),
                    path: "/tmp/b".into(),
                    group_key: "opencode".into(),
                    state: SessionState::Working,
                    agent: "opencode".into(),
                    issue: None,
                    ..Default::default()
                },
            ],
            GroupBy::Agent,
        );
        let tree = tree_from_grouped(grouped);
        assert_eq!(tree.nodes.len(), 2);
        assert!(tree.unassigned.is_empty());
    }

    #[test]
    fn build_puts_sessions_under_project() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("myapp");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let cfg = ProjectsConfig {
            discover: DiscoverMode::GitOnly,
            max_depth: 4,
            roots: vec![ProjectRoot {
                path: tmp.path().to_path_buf(),
                label: "dev".into(),
            }],
        };
        // Cannot mock list_tmux_sessions easily; test assignment logic via project_for_path + manual tree
        let session_path = repo.to_string_lossy().to_string();
        let projects = discover_projects(&cfg);
        assert_eq!(projects.len(), 1);
        let proj = project_for_path(&session_path, &projects).unwrap();
        assert_eq!(proj.name, "myapp");
    }
}
