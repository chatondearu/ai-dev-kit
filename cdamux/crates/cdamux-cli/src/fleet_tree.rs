//! Pure fleet tree model: rows, flattening, folding and selection helpers.

use cdamux_core::{
    load_orch_run_status, project_has_orch_artifacts, project_key, EnrichedSession, GroupBy,
    ProjectSessionTree, ProjectTreeNode, SessionState,
};
use std::collections::HashSet;

pub(crate) fn tree_node_key(node: &ProjectTreeNode, group_by: GroupBy) -> String {
    if group_by == GroupBy::Project {
        project_key(&node.project.path)
    } else {
        format!("grp:{}", node.project.name)
    }
}

pub(crate) fn session_passes_waiting_filter(session: &EnrichedSession, waiting_only: bool) -> bool {
    !waiting_only
        || matches!(
            session.state,
            SessionState::NeedsInput | SessionState::Error
        )
}

#[derive(Debug, Clone)]
pub(crate) enum TreeRow {
    Project {
        key: String,
        name: String,
        path: String,
        session_count: usize,
        worker_count: usize,
        expanded: bool,
    },
    Session(EnrichedSession),
    /// Headless orch worker from `.orch/run/status.json` (no tmux session).
    Worker {
        id: String,
        phase: String,
        message: String,
        project_path: String,
    },
    UnassignedHeader,
    Unassigned(EnrichedSession),
}

pub(crate) fn flatten_tree(
    tree: &ProjectSessionTree,
    expanded: &HashSet<String>,
    group_by: GroupBy,
    waiting_only: bool,
) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for node in &tree.nodes {
        let visible: Vec<EnrichedSession> = node
            .sessions
            .iter()
            .filter(|s| session_passes_waiting_filter(s, waiting_only))
            .cloned()
            .collect();
        let workers = load_orch_run_status(&node.project.path)
            .map(|s| s.workers)
            .unwrap_or_default();
        let visible_workers: Vec<_> = if waiting_only {
            workers
                .into_iter()
                .filter(|w| w.phase == "blocked")
                .collect()
        } else {
            workers
        };
        if waiting_only && visible.is_empty() && visible_workers.is_empty() {
            continue;
        }
        let key = tree_node_key(node, group_by);
        let is_exp = expanded.contains(&key);
        let project_path = node.project.path.to_string_lossy().to_string();
        rows.push(TreeRow::Project {
            key,
            name: node.project.name.clone(),
            path: project_path.clone(),
            session_count: visible.len(),
            worker_count: visible_workers.len(),
            expanded: is_exp,
        });
        if is_exp {
            for w in visible_workers {
                rows.push(TreeRow::Worker {
                    id: w.id,
                    phase: w.phase,
                    message: w.message,
                    project_path: project_path.clone(),
                });
            }
            for s in visible {
                rows.push(TreeRow::Session(s));
            }
        }
    }
    let unassigned: Vec<EnrichedSession> = tree
        .unassigned
        .iter()
        .filter(|s| session_passes_waiting_filter(s, waiting_only))
        .cloned()
        .collect();
    if !unassigned.is_empty() {
        rows.push(TreeRow::UnassignedHeader);
        for s in unassigned {
            rows.push(TreeRow::Unassigned(s));
        }
    }
    rows
}

pub(crate) fn toggle_fold_all(tree: &ProjectSessionTree, expanded: &mut HashSet<String>, group_by: GroupBy) {
    let keys: Vec<String> = tree
        .nodes
        .iter()
        .filter(|n| !n.sessions.is_empty() || project_has_orch_artifacts(&n.project.path))
        .map(|n| tree_node_key(n, group_by))
        .collect();
    let all_open = !keys.is_empty() && keys.iter().all(|k| expanded.contains(k));
    if all_open {
        expanded.clear();
    } else {
        for k in keys {
            expanded.insert(k);
        }
    }
}

pub(crate) fn session_at_row(rows: &[TreeRow], idx: usize) -> Option<&EnrichedSession> {
    match rows.get(idx)? {
        TreeRow::Session(s) | TreeRow::Unassigned(s) => Some(s),
        _ => None,
    }
}

pub(crate) fn selectable_indices(rows: &[TreeRow]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| {
            matches!(
                r,
                TreeRow::Project { .. }
                    | TreeRow::Session(_)
                    | TreeRow::Worker { .. }
                    | TreeRow::Unassigned(_)
            )
        })
        .map(|(i, _)| i)
        .collect()
}

pub(crate) fn orch_repo_at_row(rows: &[TreeRow], idx: usize) -> Option<String> {
    match rows.get(idx)? {
        TreeRow::Project { path, .. } => Some(path.clone()),
        TreeRow::Worker { project_path, .. } => Some(project_path.clone()),
        TreeRow::Session(s) | TreeRow::Unassigned(s) => Some(s.path.clone()),
        TreeRow::UnassignedHeader => None,
    }
}

pub(crate) fn fleet_attention_hint(rows: &[TreeRow]) -> String {
    let mut need_input = 0usize;
    let mut blocked = 0usize;
    for r in rows {
        match r {
            TreeRow::Session(s) | TreeRow::Unassigned(s)
                if matches!(
                    s.state,
                    SessionState::NeedsInput | SessionState::Error
                ) =>
            {
                need_input += 1;
            }
            TreeRow::Worker { phase, .. } if phase == "blocked" => blocked += 1,
            _ => {}
        }
    }
    match (need_input, blocked) {
        (0, 0) => String::new(),
        (n, 0) => format!("{n} session(s) need input"),
        (0, b) => format!("{b} orch worker(s) blocked"),
        (n, b) => format!("{n} need input · {b} orch blocked"),
    }
}

pub(crate) fn clamp_selection(rows: &[TreeRow], selected_row: usize) -> usize {
    let indices = selectable_indices(rows);
    if indices.is_empty() {
        return 0;
    }
    if indices.contains(&selected_row) {
        return selected_row;
    }
    indices[0]
}

/// Keep the selection on the same session after the rows were rebuilt;
/// fall back to clamping the index when it is gone (or nothing was anchored).
pub(crate) fn reanchor_selection(
    rows: &[TreeRow],
    selected_row: usize,
    anchor: Option<&str>,
) -> usize {
    anchor
        .and_then(|name| row_index_for_session(rows, name))
        .unwrap_or_else(|| clamp_selection(rows, selected_row))
}

pub(crate) fn move_selection(rows: &[TreeRow], selected_row: usize, delta: i32) -> usize {
    let indices = selectable_indices(rows);
    if indices.is_empty() {
        return 0;
    }
    let pos = indices
        .iter()
        .position(|&i| i == selected_row)
        .unwrap_or(0);
    let next = (pos as i32 + delta).clamp(0, indices.len() as i32 - 1);
    indices[next as usize]
}

pub(crate) fn default_expanded(tree: &ProjectSessionTree, group_by: GroupBy) -> HashSet<String> {
    tree.nodes
        .iter()
        .filter(|n| !n.sessions.is_empty())
        .map(|n| tree_node_key(n, group_by))
        .collect()
}

pub(crate) fn row_index_for_session(rows: &[TreeRow], name: &str) -> Option<usize> {
    rows.iter().position(|r| match r {
        TreeRow::Session(s) | TreeRow::Unassigned(s) => s.name == name,
        _ => false,
    })
}

pub(crate) fn focus_session_in_tree(
    tree: &ProjectSessionTree,
    expanded: &mut HashSet<String>,
    name: &str,
    group_by: GroupBy,
    waiting_only: bool,
) -> usize {
    for node in &tree.nodes {
        if node.sessions.iter().any(|s| s.name == name) {
            expanded.insert(tree_node_key(node, group_by));
            let rows = flatten_tree(tree, expanded, group_by, waiting_only);
            if let Some(idx) = row_index_for_session(&rows, name) {
                return idx;
            }
        }
    }
    if tree.unassigned.iter().any(|s| s.name == name) {
        let rows = flatten_tree(tree, expanded, group_by, waiting_only);
        return row_index_for_session(&rows, name).unwrap_or(0);
    }
    0
}

pub(crate) fn issue_at_row(rows: &[TreeRow], idx: usize) -> Option<(String, u32)> {
    match rows.get(idx)? {
        TreeRow::Session(s) | TreeRow::Unassigned(s) => s.issue.map(|n| (s.path.clone(), n)),
        TreeRow::Worker {
            id, project_path, ..
        } => {
            let n = id
                .parse()
                .ok()
                .or_else(|| cdamux_core::parse_issue_number(id))?;
            Some((project_path.clone(), n))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdamux_core::{Project, SessionState};
    use std::path::PathBuf;

    #[test]
    fn flatten_respects_expand() {
        let tree = ProjectSessionTree {
            nodes: vec![cdamux_core::ProjectTreeNode {
                project: Project {
                    name: "p".into(),
                    path: PathBuf::from("/tmp/p"),
                    root_label: "dev".into(),
                },
                sessions: vec![cdamux_core::EnrichedSession {
                    name: "s1".into(),
                    path: "/tmp/p".into(),
                    group_key: "p".into(),
                    state: SessionState::Idle,
                    agent: "—".into(),
                    issue: None,
                }],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = HashSet::new();
        let collapsed = flatten_tree(&tree, &exp, GroupBy::Project, false);
        assert_eq!(collapsed.len(), 1);
        let exp2: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let open = flatten_tree(&tree, &exp2, GroupBy::Project, false);
        assert_eq!(open.len(), 2);
    }

    fn sess(name: &str) -> cdamux_core::EnrichedSession {
        cdamux_core::EnrichedSession {
            name: name.into(),
            path: "/tmp/p".into(),
            group_key: "p".into(),
            state: SessionState::Idle,
            agent: "claude".into(),
            issue: None,
        }
    }

    #[test]
    fn reanchor_follows_session_name_not_index() {
        let rows = vec![
            TreeRow::UnassignedHeader,
            TreeRow::Unassigned(sess("c-a2")),
            TreeRow::Unassigned(sess("c-b")),
        ];
        // `c-b` was row 1 before `c-a2` appeared above it.
        assert_eq!(reanchor_selection(&rows, 1, Some("c-b")), 2);
        // Anchored session vanished: clamp the index instead.
        assert_eq!(reanchor_selection(&rows, 1, Some("gone")), 1);
        assert_eq!(reanchor_selection(&rows, 1, None), 1);
    }

    #[test]
    fn waiting_filter_hides_idle_sessions() {
        let tree = ProjectSessionTree {
            nodes: vec![cdamux_core::ProjectTreeNode {
                project: Project {
                    name: "p".into(),
                    path: PathBuf::from("/tmp/p"),
                    root_label: "dev".into(),
                },
                sessions: vec![
                    cdamux_core::EnrichedSession {
                        name: "idle".into(),
                        path: "/tmp/p".into(),
                        group_key: "p".into(),
                        state: SessionState::Idle,
                        agent: "claude".into(),
                        issue: None,
                    },
                    cdamux_core::EnrichedSession {
                        name: "wait".into(),
                        path: "/tmp/p".into(),
                        group_key: "p".into(),
                        state: SessionState::NeedsInput,
                        agent: "cursor".into(),
                        issue: None,
                    },
                ],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let rows = flatten_tree(&tree, &exp, GroupBy::Project, true);
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[1], TreeRow::Session(s) if s.name == "wait"));
    }
}
