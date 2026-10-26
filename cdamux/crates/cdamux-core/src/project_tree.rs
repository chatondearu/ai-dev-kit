//! Join discovered projects with tmux sessions for the Fleet tree UI.

use crate::session::{EnrichedSession, SessionState};
use crate::{
    discover_projects, enrich_sessions, group_sessions, list_tmux_sessions, GroupBy, GroupedSessions,
    Project, ProjectsConfig,
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

pub fn build_project_session_tree(
    projects_cfg: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
    group_by: GroupBy,
) -> ProjectSessionTree {
    let raw = list_tmux_sessions().unwrap_or_default();
    let enriched = enrich_sessions(
        &raw,
        group_by,
        projects_cfg,
        idle_seconds_for_needs_input,
    );

    if group_by != GroupBy::Project {
        return tree_from_grouped(group_sessions(enriched, group_by));
    }

    let projects = discover_projects(projects_cfg);
    let mut by_project: BTreeMap<String, Vec<EnrichedSession>> = BTreeMap::new();
    for proj in &projects {
        by_project.insert(project_key(&proj.path), Vec::new());
    }

    let mut unassigned = Vec::new();
    for session in enriched {
        match project_for_path(&session.path, &projects) {
            Some(proj) => {
                by_project
                    .entry(project_key(&proj.path))
                    .or_default()
                    .push(session);
            }
            None => unassigned.push(session),
        }
    }

    let nodes = projects
        .into_iter()
        .filter_map(|project| {
            let key = project_key(&project.path);
            let mut sessions = by_project.remove(&key).unwrap_or_default();
            if sessions.is_empty() {
                return None;
            }
            sessions.sort_by(|a, b| a.name.cmp(&b.name));
            Some(ProjectTreeNode { project, sessions })
        })
        .collect();

    unassigned.sort_by(|a, b| a.name.cmp(&b.name));

    ProjectSessionTree { nodes, unassigned }
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
        SessionState::Active => '●',
        SessionState::NeedsInput => '◆',
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
                },
                EnrichedSession {
                    name: "o-b".into(),
                    path: "/tmp/b".into(),
                    group_key: "opencode".into(),
                    state: SessionState::Active,
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
