//! Pure fleet tree model: rows, flattening, folding and selection helpers.

use cdamux_core::{
    project_has_orch_artifacts, project_key, session_forest, ChildKind, ChildRow,
    EnrichedSession, GroupBy, OrchGraphRunSummary, ProjectSessionTree, ProjectTreeNode,
    SessionForest, SessionState,
};
use crate::refresh::StatusCache;
use std::collections::HashSet;
use std::path::Path;

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
    /// Non-session child: orch run under the session that started it (or its project
    /// when headless), orch worker under its run (or its project), or CLI sub-agent under
    /// its session (`project_path` = the session path). `run` = the orch run it belongs to.
    Child {
        row: ChildRow,
        depth: usize,
        project_path: String,
        run: Option<String>,
    },
    UnassignedHeader,
}

/// Sessions of one project (or the unassigned list) with what nests under them.
struct Nest<'a> {
    sessions: &'a [EnrichedSession],
    forest: SessionForest,
    expanded: &'a HashSet<String>,
    waiting_only: bool,
    status: &'a StatusCache,
    /// Live runs of the project; empty without a repo or in waiting-only mode.
    runs: &'a [OrchGraphRunSummary],
    /// status.json workers, nested under the row of their run (`status.json` `run_id`).
    run_workers: Option<(&'a str, &'a [ChildRow])>,
    project_path: &'a str,
}

impl Nest<'_> {
    /// A row per run of `runs` (its workers, then the sessions of `idxs` tagged with it
    /// via `@cdamux_run`), then the other sessions of `idxs`, all at `depth`.
    fn level(&self, rows: &mut Vec<TreeRow>, idxs: &[usize], runs: &[&OrchGraphRunSummary], depth: usize) {
        let child = |row: ChildRow, depth, run: &str| TreeRow::Child {
            row,
            depth,
            project_path: self.project_path.to_string(),
            run: Some(run.to_string()),
        };
        let in_run = |k: usize| runs.iter().any(|r| self.sessions[k].run.as_deref() == Some(r.run_id.as_str()));
        for run in runs {
            rows.push(child(ChildRow::from_orch_run(run), depth, &run.run_id));
            if let Some((_, workers)) = self.run_workers.filter(|(id, _)| *id == run.run_id) {
                rows.extend(workers.iter().map(|w| child(w.clone(), depth + 1, &run.run_id)));
            }
            for &k in idxs.iter().filter(|&&k| self.sessions[k].run.as_deref() == Some(run.run_id.as_str())) {
                self.walk(rows, k, depth + 1);
            }
        }
        for &k in idxs.iter().filter(|&&k| !in_run(k)) {
            self.walk(rows, k, depth);
        }
    }

    /// Session `i`, then (unless collapsed) its ledger sub-agents, the runs it started
    /// and its child sessions.
    fn walk(&self, rows: &mut Vec<TreeRow>, i: usize, depth: usize) {
        let s = &self.sessions[i];
        let kids = &self.forest.children[i];
        // StatusCache holds the ledger rows (refresh scheduler): no I/O per tick.
        let subagents: &[ChildRow] = if self.waiting_only { &[] } else { self.status.subagents(&s.name) };
        let own: Vec<&OrchGraphRunSummary> =
            self.runs.iter().filter(|r| r.session.as_deref() == Some(s.name.as_str())).collect();
        let open = !self.expanded.contains(&session_fold_key(&s.name));
        rows.push(TreeRow::Session {
            session: s.clone(),
            depth,
            expanded: (!kids.is_empty() || !subagents.is_empty() || !own.is_empty()).then_some(open),
        });
        if open {
            for row in subagents {
                rows.push(TreeRow::Child {
                    row: row.clone(),
                    depth: depth + 1,
                    project_path: s.path.clone(),
                    run: None,
                });
            }
            self.level(rows, kids, &own, depth + 1);
        }
    }

    /// Root sessions, after the runs whose session is not here (headless or gone).
    fn push(&self, rows: &mut Vec<TreeRow>) {
        let here = |name: &str| self.sessions.iter().any(|s| s.name == name);
        let headless: Vec<&OrchGraphRunSummary> =
            self.runs.iter().filter(|r| !r.session.as_deref().is_some_and(here)).collect();
        self.level(rows, &self.forest.roots, &headless, 1);
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
            // Waiting-only keeps the flat list of blocked workers, without run rows.
            let runs = match status.runs(&node.project.path) {
                Some(r) if !waiting_only => r.live.as_slice(),
                _ => &[],
            };
            // Workers of a live run nest under it; others (pre-#178 status.json) stay here.
            let run_workers = status
                .status_run(&node.project.path)
                .filter(|id| runs.iter().any(|r| r.run_id == *id))
                .map(|id| (id, visible_workers.as_slice()));
            if run_workers.is_none() {
                for row in &visible_workers {
                    rows.push(TreeRow::Child {
                        row: row.clone(),
                        depth: 1,
                        project_path: project_path.clone(),
                        run: None,
                    });
                }
            }
            Nest {
                forest: session_forest(&visible),
                sessions: &visible,
                expanded,
                waiting_only,
                status,
                runs,
                run_workers,
                project_path: &project_path,
            }
            .push(&mut rows);
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
        Nest {
            forest: session_forest(&unassigned),
            sessions: &unassigned,
            expanded,
            waiting_only,
            status,
            runs: &[],
            run_workers: None,
            project_path: "",
        }
        .push(&mut rows);
    }
    rows
}

/// Which orch graph the sidecar Orch graph tab and the Graph view (`g`) show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphTarget {
    None,
    /// `.orch/graph/current.json` found from this path: project rows, pre-#178 harness.
    Latest(String),
    /// These runs (newest first, `Tab` cycles in the Graph view), found from `cwd`.
    Runs { cwd: String, runs: Vec<String> },
    /// A session that started no run, in a repo with per-run graphs.
    NoRun,
}

/// Graph of the selected row: a run row (or worker under it) shows its run; a session
/// its own live runs, else the run of its `@cdamux_run`; a project the latest run.
pub(crate) fn graph_target_at_row(
    rows: &[TreeRow],
    idx: usize,
    tree: &ProjectSessionTree,
    status: &StatusCache,
) -> GraphTarget {
    let runs_from = |cwd: &str, runs: Vec<String>| GraphTarget::Runs { cwd: cwd.to_string(), runs };
    match rows.get(idx) {
        Some(TreeRow::Child { project_path, run: Some(run), .. }) => runs_from(project_path, vec![run.clone()]),
        Some(TreeRow::Session { session: s, .. }) => {
            let repo = tree
                .nodes
                .iter()
                .find(|n| n.sessions.iter().any(|x| x.name == s.name))
                .and_then(|n| status.runs(&n.project.path).map(|r| (n.project.path.as_path(), r)));
            let own: Vec<String> = repo
                .iter()
                .flat_map(|(_, r)| &r.live)
                .filter(|r| r.session.as_deref() == Some(s.name.as_str()))
                .map(|r| r.run_id.clone())
                .collect();
            match (own.is_empty(), &s.run, repo) {
                (false, _, Some((path, _))) => runs_from(&path.to_string_lossy(), own),
                (true, Some(run), _) => runs_from(&s.path, vec![run.clone()]),
                (true, None, Some((_, r))) if r.per_run => GraphTarget::NoRun,
                _ => GraphTarget::Latest(s.path.clone()),
            }
        }
        Some(TreeRow::Project { path, .. }) | Some(TreeRow::Child { project_path: path, .. }) => {
            GraphTarget::Latest(path.clone())
        }
        _ => GraphTarget::None,
    }
}

/// Project row sidecar: live runs (with the session that started them) and recent ones.
pub(crate) fn project_runs_text(status: &StatusCache, repo: &str) -> String {
    let Some(runs) = status.runs(Path::new(repo)) else {
        return String::new();
    };
    let mut out = String::new();
    if !runs.live.is_empty() {
        out.push_str("\n\nlive runs:");
        for r in &runs.live {
            let who = r.session.as_deref().unwrap_or("headless");
            out.push_str(&format!("\n  {} · {} · {}", r.run_id, who, r.status.as_deref().unwrap_or("running")));
        }
    }
    if !runs.recent.is_empty() {
        out.push_str("\n\nrecent runs:");
        for r in &runs.recent {
            out.push_str(&format!("\n  {} · {}", r.run_id, r.status.as_deref().unwrap_or("?")));
        }
    }
    out
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

/// `h` / `l`: fold or unfold the row; no-op when it is already in that state or not foldable.
pub(crate) fn set_row_fold(rows: &[TreeRow], idx: usize, expanded: &mut HashSet<String>, open: bool) {
    let is_open = match rows.get(idx) {
        Some(TreeRow::Project { expanded, .. }) => *expanded,
        Some(TreeRow::Session { expanded: Some(o), .. }) => *o,
        _ => return,
    };
    if is_open != open {
        toggle_row_fold(rows, idx, expanded);
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
        // `l` on an open row is a no-op; `h` collapses `b`: `c` disappears,
        // selection by name lands on `orphan`. `h` again stays collapsed.
        set_row_fold(&rows, 2, &mut exp, true);
        assert!(exp.is_empty());
        set_row_fold(&rows, 2, &mut exp, false);
        let folded = flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status());
        set_row_fold(&folded, 2, &mut exp, false);
        assert_eq!(flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status()).len(), 4);
        assert_eq!(shape(&folded)[1], ("b".into(), 2, Some(false)));
        assert_eq!(folded.len(), 4);
        assert_eq!(reanchor_selection(&folded, 4, Some("orphan")), 3);
        // Focusing a hidden session reopens its parents.
        let idx = focus_session_in_tree(&tree, &mut exp, "c", GroupBy::Project, false, &no_status());
        assert!(matches!(&flatten_tree(&tree, &exp, GroupBy::Project, false, &no_status())[idx],
            TreeRow::Session { session, depth: 3, .. } if session.name == "c"));
    }

    #[test]
    fn ledger_subagents_nest_under_their_session() {
        // Only this test sets XDG_STATE_HOME; the session name is unique to it.
        let state = std::env::temp_dir().join(format!("cdamux-u3-{}", std::process::id()));
        let ledger = state.join("cdamux/agents/u3-ledger-sess.jsonl");
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        std::fs::write(&ledger, "{\"ts\":1,\"cli\":\"claude\",\"event\":\"start\",\"id\":\"ab12cd34ef\",\"name\":\"Explore\"}\n").unwrap();
        std::env::set_var("XDG_STATE_HOME", &state);
        let tree = unassigned_tree(vec![EnrichedSession {
            state: SessionState::NeedsInput,
            ..child("u3-ledger-sess", None)
        }]);
        let mut status = no_status();
        status.refresh_subagents(["u3-ledger-sess"].into_iter(), std::time::Instant::now());
        std::env::remove_var("XDG_STATE_HOME");
        let rows = flatten_tree(&tree, &HashSet::new(), GroupBy::Project, false, &status);
        let waiting = flatten_tree(&tree, &HashSet::new(), GroupBy::Project, true, &status);
        let _ = std::fs::remove_dir_all(&state);
        assert_eq!(shape(&rows), vec![("u3-ledger-sess".into(), 1, Some(true))]);
        assert!(matches!(&rows[2], TreeRow::Child { row, depth: 2, .. }
            if row.kind == ChildKind::Subagent && row.label == "Explore"));
        assert_eq!(shape(&waiting), vec![("u3-ledger-sess".into(), 1, None)]);
        assert_eq!(waiting.len(), 2, "waiting-only hides running sub-agents");
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

    /// `(depth, label)` per row: sessions by name, children as `kind id`.
    fn outline(rows: &[TreeRow]) -> Vec<(usize, String)> {
        rows.iter()
            .map(|r| match r {
                TreeRow::Project { name, .. } => (0, name.clone()),
                TreeRow::Session { session, depth, .. } => (*depth, session.name.clone()),
                TreeRow::Child { row, depth, .. } => (*depth, format!("{:?} {}", row.kind, row.id)),
                TreeRow::UnassignedHeader => (0, "unassigned".into()),
            })
            .collect()
    }

    #[test]
    fn runs_nest_under_their_session_with_workers_and_worker_sessions() {
        use crate::test_render::{fixture_runs, fixture_runs_rows, RUN_42, RUN_HEADLESS, RUN_MAIN};
        let rows = fixture_runs_rows();
        let w = format!("orch-{RUN_MAIN}-7");
        assert_eq!(
            outline(&rows),
            vec![
                (0, "alpha".into()),
                (1, format!("Run {RUN_HEADLESS}")),
                (1, "alpha-42".into()),
                (2, format!("Run {RUN_42}")),
                (1, "alpha-main".into()),
                (2, format!("Run {RUN_MAIN}")),
                (3, "Worker 7".into()),
                (3, "Worker 8".into()),
                (3, w.clone()),
            ]
        );
        // Waiting-only: no run rows, blocked workers stay flat under the project.
        let (tree, status) = fixture_runs();
        let open: HashSet<String> = ["/src/alpha".to_string()].into_iter().collect();
        let waiting = flatten_tree(&tree, &open, GroupBy::Project, true, &status);
        assert_eq!(
            outline(&waiting),
            vec![(0, "alpha".into()), (1, "Worker 8".into()), (1, "alpha-42".into())]
        );
        // Collapsing alpha-main hides its run, workers and worker session.
        let mut exp = open.clone();
        set_row_fold(&rows, 4, &mut exp, false);
        assert_eq!(flatten_tree(&tree, &exp, GroupBy::Project, false, &status).len(), 5);
    }

    #[test]
    fn graph_target_follows_run_row_own_run_then_cdamux_run() {
        use crate::test_render::{fixture_runs, fixture_runs_rows, RUN_42, RUN_MAIN};
        let (tree, status) = fixture_runs();
        let rows = fixture_runs_rows();
        let at = |i| graph_target_at_row(&rows, i, &tree, &status);
        let runs = |cwd: &str, ids: &[&str]| GraphTarget::Runs {
            cwd: cwd.into(),
            runs: ids.iter().map(|s| s.to_string()).collect(),
        };
        assert_eq!(at(0), GraphTarget::Latest("/src/alpha".into()), "project = latest");
        assert_eq!(at(3), runs("/src/alpha", &[RUN_42]), "run row");
        assert_eq!(at(2), runs("/src/alpha", &[RUN_42]), "session's own run");
        assert_eq!(at(6), runs("/src/alpha", &[RUN_MAIN]), "worker under its run");
        assert_eq!(at(8), runs("/src/alpha", &[RUN_MAIN]), "worker session via @cdamux_run");

        // Several runs from one session: newest first. A plain session: no run of its own.
        let extra = EnrichedSession { name: "solo".into(), ..sess("solo") };
        let mut tree2 = tree.clone();
        tree2.nodes[0].sessions.push(extra);
        let mut status2 = StatusCache::new(std::time::Duration::ZERO);
        let mut runs2 = status.runs(Path::new("/src/alpha")).unwrap().clone();
        let mut newer = runs2.live[1].clone();
        newer.run_id = "newer".into();
        runs2.live.insert(0, newer);
        status2.insert_fixture(Path::new("/src/alpha"), Default::default(), runs2.clone());
        let rows2 = flatten_tree(&tree2, &["/src/alpha".to_string()].into_iter().collect(), GroupBy::Project, false, &status2);
        let main = row_index_for_session(&rows2, "alpha-main").unwrap();
        let solo = row_index_for_session(&rows2, "solo").unwrap();
        assert_eq!(graph_target_at_row(&rows2, main, &tree2, &status2), runs("/src/alpha", &["newer", RUN_MAIN]));
        assert_eq!(graph_target_at_row(&rows2, solo, &tree2, &status2), GraphTarget::NoRun);
        // Pre-#178 harness (no live/ dir): the old shared graph.
        runs2.per_run = false;
        status2.insert_fixture(Path::new("/src/alpha"), Default::default(), runs2);
        assert_eq!(graph_target_at_row(&rows2, solo, &tree2, &status2), GraphTarget::Latest("/tmp/p".into()));
    }
}
