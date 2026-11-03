//! Live CDA orchestration graphs: one file per run under `.orch/graph/live/`,
//! finished runs under `runs/`, `current.json` pointing at the latest (ADR-007, #178).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::session_chrome::format_age;
use std::time::SystemTime;

/// Age after which a still-`running` graph is shown as stale in the TUI.
pub const ORCH_GRAPH_STALE_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphFile {
    pub run_id: String,
    #[serde(default)]
    pub repo: String,
    /// tmux session (`#S`) that created the run; absent for headless runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub nodes: Vec<OrchGraphNode>,
    #[serde(default)]
    pub edges: Vec<OrchGraphEdge>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphNode {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

#[derive(Debug, Clone)]
pub enum OrchGraphError {
    NotFound(PathBuf),
    Invalid { path: PathBuf, message: String },
}

impl std::fmt::Display for OrchGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrchGraphError::NotFound(p) => write!(f, "no orch graph at {}", p.display()),
            OrchGraphError::Invalid { path, message } => {
                write!(f, "invalid current.json at {}: {}", path.display(), message)
            }
        }
    }
}

pub fn orch_graph_path(repo: &Path) -> PathBuf {
    repo.join(".orch/graph/current.json")
}

fn file_age_secs(path: &Path) -> Option<u64> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    SystemTime::now().duration_since(modified).ok().map(|d| d.as_secs())
}

pub fn load_orch_graph(repo: &Path) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    load_orch_graph_at(&orch_graph_path(repo))
}

/// Parse one graph file (a run file, or `current.json` through its symlink).
pub fn load_orch_graph_at(path: &Path) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    let path = path.to_path_buf();
    if !path.is_file() {
        return Err(OrchGraphError::NotFound(path));
    }
    let raw = fs::read_to_string(&path).map_err(|e| OrchGraphError::Invalid {
        path: path.clone(),
        message: e.to_string(),
    })?;
    let graph: OrchGraphFile = serde_json::from_str(&raw).map_err(|e| OrchGraphError::Invalid {
        path: path.clone(),
        message: e.to_string(),
    })?;
    if graph.run_id.trim().is_empty() {
        return Err(OrchGraphError::Invalid {
            path,
            message: "missing field `run_id`".into(),
        });
    }
    let age = file_age_secs(&path);
    Ok((graph, age))
}

/// Walk parents from `cwd` until `.orch/graph/current.json` is found.
/// A present-but-invalid file stops the walk with [`OrchGraphError::Invalid`].
pub fn find_orch_graph_from_cwd(cwd: &str) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    if cwd.is_empty() {
        return Err(OrchGraphError::NotFound(PathBuf::from(".orch/graph/current.json")));
    }
    let mut cur = Path::new(cwd);
    loop {
        match load_orch_graph(cur) {
            Ok(loaded) => return Ok(loaded),
            Err(OrchGraphError::Invalid { path, message }) => {
                return Err(OrchGraphError::Invalid { path, message });
            }
            Err(OrchGraphError::NotFound(_)) => {}
        }
        match cur.parent() {
            Some(parent) if parent != cur => cur = parent,
            _ => {
                return Err(OrchGraphError::NotFound(orch_graph_path(Path::new(cwd))));
            }
        }
    }
}

/// True when a still-running graph has not been updated for [`ORCH_GRAPH_STALE_SECS`].
pub fn orch_graph_is_stale(graph: &OrchGraphFile, age_secs: Option<u64>) -> bool {
    let running = graph
        .status
        .as_deref()
        .map(|s| s == "running" || s.is_empty())
        .unwrap_or(true);
    if !running || graph.ended_at.is_some() {
        return false;
    }
    age_secs.is_some_and(|a| a >= ORCH_GRAPH_STALE_SECS)
}

/// ASCII layout for ratatui side panel (dynamic snapshot).
pub fn format_orch_graph_panel(graph: &OrchGraphFile, width: usize) -> String {
    format_orch_graph_panel_ex(graph, width, None)
}

/// Run header: `run`, `status`, `updated … · age · stale`, `ended`.
pub fn orch_graph_header(graph: &OrchGraphFile, age_secs: Option<u64>) -> Vec<String> {
    let mut lines = vec![format!("run {}", graph.run_id)];
    if let Some(session) = &graph.session {
        lines.push(format!("session {session}"));
    }
    if let Some(status) = &graph.status {
        if !status.is_empty() {
            lines.push(format!("status {status}"));
        }
    }
    let mut updated_line = if graph.updated_at.is_empty() {
        String::new()
    } else {
        format!("updated {}", graph.updated_at)
    };
    if let Some(age) = age_secs {
        if updated_line.is_empty() {
            updated_line = format!("mtime {} ago", format_age(age));
        } else {
            updated_line.push_str(&format!(" · {} ago", format_age(age)));
        }
    }
    if orch_graph_is_stale(graph, age_secs) {
        if updated_line.is_empty() {
            updated_line = "stale".into();
        } else {
            updated_line.push_str(" · stale");
        }
    }
    if !updated_line.is_empty() {
        lines.push(updated_line);
    }
    if let Some(ended) = &graph.ended_at {
        if !ended.is_empty() {
            lines.push(format!("ended {ended}"));
        }
    }
    lines
}

pub fn format_orch_graph_panel_ex(
    graph: &OrchGraphFile,
    width: usize,
    age_secs: Option<u64>,
) -> String {
    let w = width.clamp(40, 120);
    let mut lines = orch_graph_header(graph, age_secs);
    lines.push(String::new());

    let roots: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.parent.is_none() || n.id == "integrator")
        .collect();

    let mut shown = std::collections::BTreeSet::new();
    for root in roots {
        render_node_tree(graph, &root.id, 0, w, &mut lines, &mut shown);
    }
    for n in &graph.nodes {
        if !shown.contains(&n.id) {
            push_node_line(n, 0, w, &mut lines);
            shown.insert(n.id.clone());
        }
    }

    // Detail block: full messages for non-terminal nodes (P1.5).
    let interesting: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            !matches!(n.status.as_str(), "done" | "ok" | "skipped")
                || n.message.len() > 24
        })
        .take(8)
        .collect();
    if !interesting.is_empty() {
        lines.push(String::new());
        lines.push("detail:".to_string());
        for n in interesting {
            lines.push(truncate(
                &format!("  {} · {} · {}", n.id, n.status, n.message),
                w,
            ));
        }
    }

    if graph.edges.is_empty() && graph.nodes.is_empty() {
        lines.push("(no nodes yet)".to_string());
    } else if !graph.edges.is_empty() {
        lines.push(String::new());
        lines.push("edges:".to_string());
        for e in graph.edges.iter().take(12) {
            lines.push(format!(
                "  {} --{}→ {}",
                truncate(&e.from, 16),
                e.edge_type,
                truncate(&e.to, 16)
            ));
        }
    }

    lines.join("\n")
}

/// Append recent archived runs under `.orch/graph/runs/` (newest first).
pub fn format_orch_graph_panel_with_history(
    graph: &OrchGraphFile,
    width: usize,
    age_secs: Option<u64>,
    repo: &Path,
) -> String {
    let mut out = format_orch_graph_panel_ex(graph, width, age_secs);
    let runs = list_orch_graph_runs(repo, 5);
    if runs.is_empty() {
        return out;
    }
    out.push_str("\n\nrecent runs:\n");
    for r in runs {
        let st = r.status.unwrap_or_else(|| "?".into());
        out.push_str(&format!("  {} · {}\n", truncate(&r.run_id, 28), st));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchGraphRunSummary {
    pub run_id: String,
    pub path: PathBuf,
    pub status: Option<String>,
    /// tmux session that created the run; `None` = headless (orch-auto, no tmux).
    pub session: Option<String>,
    /// Under `live/` (or a running legacy `current.json`), not archived in `runs/`.
    pub live: bool,
}

pub fn orch_graph_runs_dir(repo: &Path) -> PathBuf {
    repo.join(".orch/graph/runs")
}

pub fn orch_graph_live_dir(repo: &Path) -> PathBuf {
    repo.join(".orch/graph/live")
}

/// Newest-first summaries from archived run JSON files.
pub fn list_orch_graph_runs(repo: &Path, limit: usize) -> Vec<OrchGraphRunSummary> {
    list_run_dir(&orch_graph_runs_dir(repo), limit, false)
}

/// Live runs, newest first. Without `live/` (pre-#178 harness) a still-running
/// `current.json` stands in as one run without a session.
pub fn list_live_orch_runs(repo: &Path) -> Vec<OrchGraphRunSummary> {
    let dir = orch_graph_live_dir(repo);
    if dir.is_dir() {
        return list_run_dir(&dir, usize::MAX, true);
    }
    match load_orch_graph(repo) {
        Ok((g, _)) if g.ended_at.is_none() && matches!(g.status.as_deref(), None | Some("" | "running")) => {
            vec![summary(g, orch_graph_path(repo), true)]
        }
        _ => Vec::new(),
    }
}

/// Run file for `run_id`: `live/`, then `runs/`, then a legacy `current.json` with that id.
/// The id is reduced to the writer's file-name charset, so it can never leave the graph dir.
pub fn find_orch_run(repo: &Path, run_id: &str) -> Option<PathBuf> {
    // Same mapping as graph.sh: `tr -c 'A-Za-z0-9._-' '_'` (bytewise).
    let name: String = run_id
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"._-".contains(&b) { b as char } else { '_' })
        .collect();
    let name = format!("{name}.json");
    [orch_graph_live_dir(repo), orch_graph_runs_dir(repo)]
        .into_iter()
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
        .or_else(|| {
            let (g, _) = load_orch_graph(repo).ok()?;
            (g.run_id == run_id).then(|| orch_graph_path(repo))
        })
}

/// [`find_orch_run`] in `cwd` or its nearest ancestor holding that run.
pub fn find_orch_run_from_cwd(cwd: &str, run_id: &str) -> Option<PathBuf> {
    if cwd.is_empty() {
        return None;
    }
    Path::new(cwd).ancestors().find_map(|a| find_orch_run(a, run_id))
}

fn summary(g: OrchGraphFile, path: PathBuf, live: bool) -> OrchGraphRunSummary {
    OrchGraphRunSummary { run_id: g.run_id, path, status: g.status, session: g.session, live }
}

fn list_run_dir(dir: &Path, limit: usize, live: bool) -> Vec<OrchGraphRunSummary> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    // `*.json` only: skips the writer's `<run>.json.lock` and `<run>.json.tmp.*` files.
    let mut entries: Vec<_> = rd
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == "json")
                .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
    entries
        .into_iter()
        .take(limit)
        .filter_map(|e| {
            let path = e.path();
            let raw = fs::read_to_string(&path).ok()?;
            let g: OrchGraphFile = serde_json::from_str(&raw).ok()?;
            Some(summary(g, path, live))
        })
        .collect()
}

fn render_node_tree(
    graph: &OrchGraphFile,
    id: &str,
    depth: usize,
    width: usize,
    lines: &mut Vec<String>,
    shown: &mut std::collections::BTreeSet<String>,
) {
    if shown.contains(id) {
        return;
    }
    let node = graph.nodes.iter().find(|n| n.id == id);
    if let Some(n) = node {
        push_node_line(n, depth, width, lines);
        shown.insert(id.to_string());
    }
    for e in graph.edges.iter().filter(|e| e.from == id) {
        render_node_tree(graph, &e.to, depth + 1, width, lines, shown);
    }
}

fn push_node_line(n: &OrchGraphNode, depth: usize, width: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    // Same glyphs as the drawn graph and the step bar.
    let icon = crate::NodeState::from_status(&n.status, false).icon();
    let head = format!(
        "{}{} {} [{}|{}] {}",
        indent,
        icon,
        truncate(&n.id, 18),
        truncate(&n.kind, 8),
        truncate(&n.phase, 10),
        truncate(&n.message, 36)
    );
    lines.push(truncate(&head, width));
}

/// Truncate to `max` chars (not bytes: ids and messages may be non-ASCII).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn format_includes_run_id() {
        let g = OrchGraphFile {
            run_id: "r1".into(),
            repo: "/tmp".into(),
            session: None,
            updated_at: "t".into(),
            nodes: vec![OrchGraphNode {
                id: "integrator".into(),
                kind: "integrator".into(),
                phase: "pending".into(),
                status: "running".into(),
                message: "wait".into(),
                parent: None,
            }],
            edges: vec![],
            status: Some("running".into()),
            ended_at: None,
        };
        let out = format_orch_graph_panel(&g, 60);
        assert!(out.contains("run r1"));
        assert!(out.contains("integrator"));
        assert!(out.contains("status running"));
    }

    #[test]
    fn truncate_counts_chars_not_bytes() {
        assert_eq!(truncate("épée", 4), "épée");
        assert_eq!(truncate("épées", 4), "épé…");
    }

    #[test]
    fn load_rejects_missing_run_id_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = orch_graph_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        write!(f, r#"{{"issue":6,"mode":"cursor-session-fallback"}}"#).unwrap();
        let err = load_orch_graph(dir.path()).unwrap_err();
        match err {
            OrchGraphError::Invalid { message, .. } => {
                assert!(
                    message.contains("run_id") || message.contains("missing field"),
                    "msg={message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn load_accepts_defaults_without_edges() {
        let dir = tempfile::tempdir().unwrap();
        let path = orch_graph_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        write!(f, r#"{{"run_id":"r2","nodes":[]}}"#).unwrap();
        let (g, _) = load_orch_graph(dir.path()).unwrap();
        assert_eq!(g.run_id, "r2");
        assert!(g.edges.is_empty());
    }

    #[test]
    fn stale_badge_when_old_running() {
        let g = OrchGraphFile {
            run_id: "r1".into(),
            repo: String::new(),
            session: None,
            updated_at: "old".into(),
            nodes: vec![],
            edges: vec![],
            status: Some("running".into()),
            ended_at: None,
        };
        let out = format_orch_graph_panel_ex(&g, 60, Some(ORCH_GRAPH_STALE_SECS + 10));
        assert!(out.contains("stale"), "{out}");
    }

    fn write(path: &Path, body: &str, age_secs: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() - std::time::Duration::from_secs(age_secs)).unwrap();
    }

    #[test]
    fn live_runs_newest_first_with_session_and_archive_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let live = orch_graph_live_dir(repo);
        write(&live.join("old.json"), r#"{"run_id":"old","session":"a","status":"running"}"#, 60);
        write(&live.join("new.json"), r#"{"run_id":"new","status":"running"}"#, 1);
        write(&live.join("new.json.lock"), "", 0);
        write(&orch_graph_runs_dir(repo).join("done.json"), r#"{"run_id":"done","status":"ok"}"#, 5);
        let runs = list_live_orch_runs(repo);
        let ids: Vec<_> = runs.iter().map(|r| (r.run_id.as_str(), r.session.as_deref(), r.live)).collect();
        assert_eq!(ids, vec![("new", None, true), ("old", Some("a"), true)]);
        assert!(!list_orch_graph_runs(repo, 5)[0].live);
        assert_eq!(find_orch_run(repo, "old"), Some(live.join("old.json")));
        assert_eq!(find_orch_run(repo, "done"), Some(orch_graph_runs_dir(repo).join("done.json")));
        // From a worktree below the repo; traversal-looking ids stay inside the graph dir.
        let wt = repo.join(".orch/worktrees/7-x");
        fs::create_dir_all(&wt).unwrap();
        assert_eq!(find_orch_run_from_cwd(wt.to_str().unwrap(), "new"), Some(live.join("new.json")));
        assert_eq!(find_orch_run(repo, "../live/new"), None);
    }

    #[test]
    fn legacy_current_json_stands_in_only_while_running() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        write(&orch_graph_path(repo), r#"{"run_id":"legacy","status":"running"}"#, 1);
        let runs = list_live_orch_runs(repo);
        assert_eq!(runs.len(), 1);
        assert_eq!((runs[0].run_id.as_str(), runs[0].session.as_deref()), ("legacy", None));
        assert_eq!(find_orch_run(repo, "legacy"), Some(orch_graph_path(repo)));
        write(&orch_graph_path(repo), r#"{"run_id":"legacy","status":"ok","ended_at":"t"}"#, 1);
        assert!(list_live_orch_runs(repo).is_empty());
        // Once `live/` exists, current.json is only the pointer: not listed twice.
        write(&orch_graph_path(repo), r#"{"run_id":"legacy","status":"running"}"#, 1);
        fs::create_dir_all(orch_graph_live_dir(repo)).unwrap();
        assert!(list_live_orch_runs(repo).is_empty());
    }
}
