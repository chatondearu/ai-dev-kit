//! Pure fleet tree model: rows, flattening, folding and selection helpers.

use cdamux_core::{
    project_has_orch_artifacts, project_key, session_forest, ChildKind, ChildRow,
    EnrichedSession, GroupBy, ProjectSessionTree, ProjectTreeNode, SessionForest, SessionState,
};
use crate::refresh::StatusCache;
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

/// Fold key of a session with child sessions; present in the `expanded` set = collapsed,
/// so new parents start open.
pub(crate) fn session_fold_key(name: &str) -> String {
    format!("fold:{name}")
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
    /// tmux session; `depth` 1 sits under its project (or the unassigned header), deeper
    /// rows under their `@cdamux_parent`. `expanded` is `None` without child sessions.
    Session {
        session: EnrichedSession,
        depth: usize,
        expanded: Option<bool>,
    },
    /// Non-session child (headless orch worker today) under its project.
    Child {
        row: ChildRow,
        depth: usize,
        project_path: String,
    },
    UnassignedHeader,
}

/// Push `sessions` nested by parent, skipping the children of collapsed sessions.
fn push_sessions(rows: &mut Vec<TreeRow>, sessions: &[EnrichedSession], expanded: &HashSet<String>) {
    fn walk(
        rows: &mut Vec<TreeRow>,
        sessions: &[EnrichedSession],
        forest: &SessionForest,
        expanded: &HashSet<String>,
        i: usize,
        depth: usize,
    ) {
        let kids = &forest.children[i];
        let open = !expanded.contains(&session_fold_key(&sessions[i].name));
        rows.push(TreeRow::Session {
            session: sessions[i].clone(),
            depth,
            expanded: (!kids.is_empty()).then_some(open),
        });
        if open {
            for &k in kids {
                walk(rows, sessions, forest, expanded, k, depth + 1);
            }
        }
    }
    let forest = session_forest(sessions);
    for &r in &forest.roots {
        walk(rows, sessions, &forest, expanded, r, 1);
    }
}

pub(crate) fn flatten_tree(
    tree: &ProjectSessionTree,
    expanded: &HashSet<String>,
    group_by: GroupBy,
    waiting_only: bool,
    status: &StatusCache,
) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    // Live tmux sessions, so worker rows preview the pane only while it exists.
    let live: HashSet<&str> = tree
        .nodes
        .iter()
        .flat_map(|n| &n.sessions)
        .chain(&tree.unassigned)
        .map(|s| s.name.as_str())
        .collect();
    for node in &tree.nodes {
        let visible: Vec<EnrichedSession> = node
            .sessions
            .iter()
            .filter(|s| session_passes_waiting_filter(s, waiting_only))
            .cloned()
            .collect();
        let visible_workers: Vec<ChildRow> = status
            .workers(&node.project.path)
            .iter()
            .map(|w| {
                let live = w.tmux_session.as_deref().is_some_and(|s| live.contains(s));
                ChildRow::from_orch_worker(w, live)
            })
            .filter(|w| !waiting_only || w.state == SessionState::Error)
            .collect();
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
            for row in visible_workers {
                rows.push(TreeRow::Child {
                    row,
                    depth: 1,
                    project_path: project_path.clone(),
                });
            }
            push_sessions(&mut rows, &visible, expanded);
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
        push_sessions(&mut rows, &unassigned, expanded);
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

/// Expand/collapse the project or parent session at `idx` (no-op on other rows).
pub(crate) fn toggle_row_fold(rows: &[TreeRow], idx: usize, expanded: &mut HashSet<String>) {
    // Projects store "open" keys, parent sessions store "collapsed" keys.
    let (key, insert) = match rows.get(idx) {
        Some(TreeRow::Project { key, expanded: open, .. }) => (key.clone(), !*open),
        Some(TreeRow::Session {
            session,
            expanded: Some(open),
            ..
        }) => (session_fold_key(&session.name), *open),
        _ => return,
    };
    if insert {
        expanded.insert(key);
    } else {
        expanded.remove(&key);
    }
}

pub(crate) fn session_at_row(rows: &[TreeRow], idx: usize) -> Option<&EnrichedSession> {
    match rows.get(idx)? {
        TreeRow::Session { session, .. } => Some(session),
        _ => None,
    }
}

pub(crate) fn selectable_indices(rows: &[TreeRow]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| !matches!(r, TreeRow::UnassignedHeader))
        .map(|(i, _)| i)
        .collect()
}

pub(crate) fn orch_repo_at_row(rows: &[TreeRow], idx: usize) -> Option<String> {
    match rows.get(idx)? {
        TreeRow::Project { path, .. } => Some(path.clone()),
        TreeRow::Child { project_path, .. } => Some(project_path.clone()),
        TreeRow::Session { session, .. } => Some(session.path.clone()),
        TreeRow::UnassignedHeader => None,
    }
}

pub(crate) fn fleet_attention_hint(rows: &[TreeRow]) -> String {
    let mut need_input = 0usize;
    let mut blocked = 0usize;
    for r in rows {
        match r {
            TreeRow::Session { session, .. }
                if matches!(
                    session.state,
                    SessionState::NeedsInput | SessionState::Error
                ) =>
            {
                need_input += 1;
            }
            TreeRow::Child { row, .. }
                if row.kind == ChildKind::Worker && row.state == SessionState::Error =>
            {
                blocked += 1
            }
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
        TreeRow::Session { session, .. } => session.name == name,
        _ => false,
    })
}

/// Open the collapsed parents of `name` so the session gets a row.
fn unfold_ancestors(sessions: &[EnrichedSession], name: &str, expanded: &mut HashSet<String>) {
    let parent_of = |n: &str| sessions.iter().find(|s| s.name == n)?.parent.clone();
    let mut cur = parent_of(name);
    // Bounded walk: a parent cycle cannot loop forever.
    for _ in 0..sessions.len() {
        let Some(p) = cur else { break };
        expanded.remove(&session_fold_key(&p));
        cur = parent_of(&p);
    }
}

pub(crate) fn focus_session_in_tree(
    tree: &ProjectSessionTree,
    expanded: &mut HashSet<String>,
    name: &str,
    group_by: GroupBy,
    waiting_only: bool,
    status: &StatusCache,
) -> usize {
    for node in &tree.nodes {
        if node.sessions.iter().any(|s| s.name == name) {
            expanded.insert(tree_node_key(node, group_by));
            unfold_ancestors(&node.sessions, name, expanded);
            let rows = flatten_tree(tree, expanded, group_by, waiting_only, status);
            if let Some(idx) = row_index_for_session(&rows, name) {
                return idx;
            }
        }
    }
    if tree.unassigned.iter().any(|s| s.name == name) {
        unfold_ancestors(&tree.unassigned, name, expanded);
        let rows = flatten_tree(tree, expanded, group_by, waiting_only, status);
        return row_index_for_session(&rows, name).unwrap_or(0);
    }
    0
}

pub(crate) fn issue_at_row(rows: &[TreeRow], idx: usize) -> Option<(String, u32)> {
    match rows.get(idx)? {
        TreeRow::Session { session: s, .. } => s.issue.map(|n| (s.path.clone(), n)),
        TreeRow::Child {
            row, project_path, ..
        } if row.kind == ChildKind::Worker => {
            let n = row
                .id
                .parse()
                .ok()
                .or_else(|| cdamux_core::parse_issue_number(&row.id))?;
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

    fn no_status() -> StatusCache {
        StatusCache::new(std::time::Duration::ZERO)
    }

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
                    ..Default::default()
                }],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = HashSet::new();
        let collapsed = flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status());
        assert_eq!(collapsed.len(), 1);
        let exp2: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let open = flatten_tree(&tree, &exp2, GroupBy::Project, false, &no_status());
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
            ..Default::default()
        }
    }

    #[test]
    fn reanchor_follows_session_name_not_index() {
        let rows = vec![
            TreeRow::UnassignedHeader,
            row(sess("c-a2")),
            row(sess("c-b")),
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
                        ..Default::default()
                    },
                    cdamux_core::EnrichedSession {
                        name: "wait".into(),
                        path: "/tmp/p".into(),
                        group_key: "p".into(),
                        state: SessionState::NeedsInput,
                        agent: "cursor".into(),
                        issue: None,
                        ..Default::default()
                    },
                ],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let rows = flatten_tree(&tree, &exp, GroupBy::Project, true, &no_status());
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[1], TreeRow::Session { session, .. } if session.name == "wait"));
    }

    fn row(session: EnrichedSession) -> TreeRow {
        TreeRow::Session {
            session,
            depth: 1,
            expanded: None,
        }
    }

    fn child(name: &str, parent: Option<&str>) -> EnrichedSession {
        EnrichedSession {
            parent: parent.map(Into::into),
            ..sess(name)
        }
    }

    fn unassigned_tree(sessions: Vec<EnrichedSession>) -> ProjectSessionTree {
        ProjectSessionTree {
            nodes: vec![],
            unassigned: sessions,
        }
    }

    /// `(name, depth, expanded)` per session row.
    fn shape(rows: &[TreeRow]) -> Vec<(String, usize, Option<bool>)> {
        rows.iter()
            .filter_map(|r| match r {
                TreeRow::Session {
                    session,
                    depth,
                    expanded,
                } => Some((session.name.clone(), *depth, *expanded)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn nests_sessions_three_deep_and_folds() {
        let tree = unassigned_tree(vec![
            child("a", None),
            child("b", Some("a")),
            child("c", Some("b")),
            child("orphan", Some("dead")),
        ]);
        let mut exp = HashSet::new();
        let rows = flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status());
        assert_eq!(
            shape(&rows),
            vec![
                ("a".into(), 1, Some(true)),
                ("b".into(), 2, Some(true)),
                ("c".into(), 3, None),
                ("orphan".into(), 1, None),
            ]
        );
        // Collapse `b`: `c` disappears, selection by name lands on `orphan`.
        toggle_row_fold(&rows, 2, &mut exp);
        let folded = flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status());
        assert_eq!(shape(&folded)[1], ("b".into(), 2, Some(false)));
        assert_eq!(folded.len(), 4);
        assert_eq!(reanchor_selection(&folded, 4, Some("orphan")), 3);
        // Focusing a hidden session reopens its parents.
        let idx = focus_session_in_tree(&tree, &mut exp, "c", GroupBy::Project, false, &no_status());
        assert!(matches!(&flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status())[idx],
            TreeRow::Session { session, depth: 3, .. } if session.name == "c"));
    }

    #[test]
    fn parent_cycle_keeps_every_session_once() {
        let tree = unassigned_tree(vec![child("a", Some("b")), child("b", Some("a"))]);
        let rows = flatten_tree(&tree, &HashSet::new(), GroupBy::Project, false, &no_status());
        assert_eq!(
            shape(&rows),
            vec![("a".into(), 1, None), ("b".into(), 1, None)]
        );
    }
}
