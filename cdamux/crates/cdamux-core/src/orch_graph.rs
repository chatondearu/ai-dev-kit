//! Orch graphs under `$CDAMUX_HOME/runs/<run_id>.json` (ADR-011 §5 hard cut #252).
//! Live vs archived is the run's `status` / `ended_at` (no in-repo `.orch/graph`).

use crate::paths::{cdamux_home, cdamux_state_dir};
use crate::session_chrome::format_age;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
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
                write!(f, "invalid run graph at {}: {}", path.display(), message)
            }
        }
    }
}

/// `$CDAMUX_HOME/runs` (global). `repo` is unused; kept for call-site compatibility.
pub fn orch_graph_runs_dir(_repo: &Path) -> PathBuf {
    cdamux_state_dir("runs").unwrap_or_else(|| PathBuf::from("/nonexistent/cdamux/runs"))
}

/// Same directory as archived runs — live is filtered by status (hard cut #252).
pub fn orch_graph_live_dir(repo: &Path) -> PathBuf {
    orch_graph_runs_dir(repo)
}

/// Path of the newest run file for `repo`, or the runs dir when none exist.
pub fn orch_graph_path(repo: &Path) -> PathBuf {
    list_live_orch_runs(repo)
        .into_iter()
        .chain(list_orch_graph_runs(repo, 1))
        .next()
        .map(|r| r.path)
        .unwrap_or_else(|| orch_graph_runs_dir(repo))
}

/// True when the run's `repo` field names `want` (canonical when possible).
pub fn run_repo_matches(run_repo: &str, want: &Path) -> bool {
    if run_repo.is_empty() {
        return false;
    }
    let run_p = Path::new(run_repo);
    if run_p == want {
        return true;
    }
    match (run_p.canonicalize(), want.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => run_p == want,
    }
}

fn file_age_secs(path: &Path) -> Option<u64> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    SystemTime::now().duration_since(modified).ok().map(|d| d.as_secs())
}

pub fn load_orch_graph(repo: &Path) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    let path = list_live_orch_runs(repo)
        .into_iter()
        .chain(list_orch_graph_runs(repo, 1))
        .next()
        .map(|r| r.path)
        .ok_or_else(|| {
            OrchGraphError::NotFound(
                cdamux_home()
                    .map(|h| h.join("runs"))
                    .unwrap_or_else(|| PathBuf::from("$CDAMUX_HOME/runs")),
            )
        })?;
    load_orch_graph_at(&path)
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

/// Graph for a tmux session: newest live run with `session == name`, else `@cdamux_run`.
/// Does **not** fall back to the repo's latest run (#236 item 4).
pub fn find_orch_graph_for_session(
    repo: &Path,
    session: &str,
    cdamux_run: Option<&str>,
) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    if let Some(summary) = list_live_orch_runs(repo)
        .into_iter()
        .find(|r| r.session.as_deref() == Some(session))
    {
        return load_orch_graph_at(&summary.path);
    }
    if let Some(run_id) = cdamux_run.filter(|s| !s.is_empty()) {
        if let Some(path) = find_orch_run(repo, run_id) {
            return load_orch_graph_at(&path);
        }
    }
    Err(OrchGraphError::NotFound(
        cdamux_home()
            .map(|h| h.join("runs"))
            .unwrap_or_else(|| PathBuf::from("$CDAMUX_HOME/runs")),
    ))
}

/// Like [`find_orch_graph_for_session`] walking parents from `cwd` for the repo root.
pub fn find_orch_graph_for_session_from_cwd(
    cwd: &str,
    session: &str,
    cdamux_run: Option<&str>,
) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    if cwd.is_empty() {
        return Err(OrchGraphError::NotFound(PathBuf::from("$CDAMUX_HOME/runs")));
    }
    let mut last_nf = OrchGraphError::NotFound(PathBuf::from("$CDAMUX_HOME/runs"));
    for cur in Path::new(cwd).ancestors() {
        match find_orch_graph_for_session(cur, session, cdamux_run) {
            Ok(loaded) => return Ok(loaded),
            Err(OrchGraphError::Invalid { path, message }) => {
                return Err(OrchGraphError::Invalid { path, message });
            }
            Err(e @ OrchGraphError::NotFound(_)) => last_nf = e,
        }
    }
    Err(last_nf)
}

/// Walk parents from `cwd` until a `$CDAMUX_HOME` run matches that repo path.
/// A present-but-invalid file stops the walk with [`OrchGraphError::Invalid`].
pub fn find_orch_graph_from_cwd(cwd: &str) -> Result<(OrchGraphFile, Option<u64>), OrchGraphError> {
    if cwd.is_empty() {
        return Err(OrchGraphError::NotFound(PathBuf::from("$CDAMUX_HOME/runs")));
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

/// Append recent finished runs under `$CDAMUX_HOME/runs/` (newest first).
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

/// Newest-first finished runs for `repo` under `$CDAMUX_HOME/runs/`.
pub fn list_orch_graph_runs(repo: &Path, limit: usize) -> Vec<OrchGraphRunSummary> {
    list_runs_for_repo(repo, limit, false)
}

/// Live (still-running) runs for `repo`, newest first.
pub fn list_live_orch_runs(repo: &Path) -> Vec<OrchGraphRunSummary> {
    list_runs_for_repo(repo, usize::MAX, true)
}

/// Run file for `run_id` under `$CDAMUX_HOME/runs/` when it belongs to `repo`.
pub fn find_orch_run(repo: &Path, run_id: &str) -> Option<PathBuf> {
    let name: String = run_id
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"._-".contains(&b) {
                b as char
            } else {
                '_'
            }
        })
        .collect();
    let path = orch_graph_runs_dir(repo).join(format!("{name}.json"));
    if !path.is_file() {
        return None;
    }
    let (g, _) = load_orch_graph_at(&path).ok()?;
    run_repo_matches(&g.repo, repo).then_some(path)
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

fn is_live_run(g: &OrchGraphFile) -> bool {
    g.ended_at.is_none()
        && matches!(g.status.as_deref(), None | Some("" | "running"))
}

/// One parsed run under `$CDAMUX_HOME/runs/`, newest-first in the index.
struct IndexedRun {
    repo: String,
    summary: OrchGraphRunSummary,
}

/// Cheap fingerprint of `runs/`: dir mtime + json count + newest file mtime.
/// File rewrites bump file mtime without always bumping the directory mtime.
#[derive(Clone, Copy, PartialEq, Eq)]
struct DirStamp {
    dir_mtime: Option<SystemTime>,
    json_count: usize,
    newest_file: Option<SystemTime>,
}

fn dir_stamp(dir: &Path) -> DirStamp {
    let dir_mtime = fs::metadata(dir).ok().and_then(|m| m.modified().ok());
    let mut json_count = 0;
    let mut newest_file = None;
    if let Ok(rd) = fs::read_dir(dir) {
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            json_count += 1;
            if let Some(m) = ent.metadata().ok().and_then(|m| m.modified().ok()) {
                if newest_file.is_none_or(|n| m > n) {
                    newest_file = Some(m);
                }
            }
        }
    }
    DirStamp {
        dir_mtime,
        json_count,
        newest_file,
    }
}

struct RunsIndex {
    home: PathBuf,
    stamp: DirStamp,
    /// Newest-first (by file mtime).
    entries: Vec<IndexedRun>,
}

#[cfg(test)]
static RUN_FILE_PARSE_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// How many run JSON files were parsed since the last [`test_reset_run_file_parse_count`].
#[cfg(test)]
pub fn test_run_file_parse_count() -> usize {
    RUN_FILE_PARSE_COUNT.load(std::sync::atomic::Ordering::SeqCst)
}

#[cfg(test)]
pub fn test_reset_run_file_parse_count() {
    RUN_FILE_PARSE_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
}

fn runs_index_lock() -> std::sync::MutexGuard<'static, Option<RunsIndex>> {
    static CACHE: OnceLock<Mutex<Option<RunsIndex>>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn rebuild_runs_index(home: PathBuf, stamp: DirStamp) -> RunsIndex {
    let dir = home.join("runs");
    let mut dated: Vec<(Option<SystemTime>, IndexedRun)> = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let mtime = ent.metadata().ok().and_then(|m| m.modified().ok());
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(g) = serde_json::from_str::<OrchGraphFile>(&raw) else {
                continue;
            };
            #[cfg(test)]
            RUN_FILE_PARSE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let live = is_live_run(&g);
            dated.push((
                mtime,
                IndexedRun {
                    repo: g.repo.clone(),
                    summary: summary(g, path, live),
                },
            ));
        }
    }
    dated.sort_by_key(|(m, _)| std::cmp::Reverse(*m));
    RunsIndex {
        home,
        stamp,
        entries: dated.into_iter().map(|(_, e)| e).collect(),
    }
}

/// Newest-first runs for `repo`, filtered by live/archived. Parses each JSON at most
/// once per `$CDAMUX_HOME/runs` fingerprint so Fleet refresh over many projects
/// does not re-parse the whole dir per project (Alt-P / #295).
fn list_runs_for_repo(repo: &Path, limit: usize, live_only: bool) -> Vec<OrchGraphRunSummary> {
    let Some(home) = cdamux_home() else {
        return Vec::new();
    };
    let stamp = dir_stamp(&home.join("runs"));
    let mut guard = runs_index_lock();
    let needs_rebuild = match guard.as_ref() {
        Some(idx) => idx.home != home || idx.stamp != stamp,
        None => true,
    };
    if needs_rebuild {
        *guard = Some(rebuild_runs_index(home, stamp));
    }
    let Some(idx) = guard.as_ref() else {
        return Vec::new();
    };
    idx.entries
        .iter()
        .filter(|e| {
            if live_only != e.summary.live {
                return false;
            }
            run_repo_matches(&e.repo, repo)
        })
        .map(|e| e.summary.clone())
        .take(limit)
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
        let _g = crate::paths::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("cdamux");
        let repo = dir.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        let path = home.join("runs/bad.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        write!(f, r#"{{"issue":6,"mode":"cursor-session-fallback","repo":"{}"}}"#, repo.display()).unwrap();
        let err = load_orch_graph_at(&path).unwrap_err();
        match err {
            OrchGraphError::Invalid { message, .. } => {
                assert!(
                    message.contains("run_id") || message.contains("missing field"),
                    "msg={message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn load_accepts_defaults_without_edges() {
        let _g = crate::paths::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("cdamux");
        let repo = dir.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        let path = home.join("runs/r2.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let body = format!(
            r#"{{"run_id":"r2","repo":"{}","nodes":[],"status":"running"}}"#,
            repo.display()
        );
        fs::write(&path, body).unwrap();
        let (g, _) = load_orch_graph(&repo).unwrap();
        assert_eq!(g.run_id, "r2");
        assert!(g.edges.is_empty());
        std::env::remove_var("CDAMUX_HOME");
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
        let _g = crate::paths::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("cdamux");
        let repo = dir.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        let runs = home.join("runs");
        let repo_s = repo.display().to_string();
        write(
            &runs.join("old.json"),
            &format!(r#"{{"run_id":"old","repo":"{repo_s}","session":"a","status":"running"}}"#),
            60,
        );
        write(
            &runs.join("new.json"),
            &format!(r#"{{"run_id":"new","repo":"{repo_s}","status":"running"}}"#),
            1,
        );
        write(&runs.join("new.json.lock"), "", 0);
        write(
            &runs.join("done.json"),
            &format!(r#"{{"run_id":"done","repo":"{repo_s}","status":"ok","ended_at":"t"}}"#),
            5,
        );
        let live = list_live_orch_runs(&repo);
        let ids: Vec<_> = live
            .iter()
            .map(|r| (r.run_id.as_str(), r.session.as_deref(), r.live))
            .collect();
        assert_eq!(ids, vec![("new", None, true), ("old", Some("a"), true)]);
        assert!(!list_orch_graph_runs(&repo, 5)[0].live);
        assert_eq!(find_orch_run(&repo, "old"), Some(runs.join("old.json")));
        assert_eq!(find_orch_run(&repo, "done"), Some(runs.join("done.json")));
        let wt = repo.join(".orch/worktrees/7-x");
        fs::create_dir_all(&wt).unwrap();
        assert_eq!(
            find_orch_run_from_cwd(wt.to_str().unwrap(), "new"),
            Some(runs.join("new.json"))
        );
        assert_eq!(find_orch_run(&repo, "../live/new"), None);
        // Session API: prefer that session's live run, not the newest repo run (#236.4).
        let (g, _) = find_orch_graph_for_session(&repo, "a", None).unwrap();
        assert_eq!(g.run_id, "old");
        let (g2, _) = find_orch_graph_for_session(&repo, "other", Some("done")).unwrap();
        assert_eq!(g2.run_id, "done");
        assert!(find_orch_graph_for_session(&repo, "missing", None).is_err());
        assert!(find_orch_graph_for_session(&repo, "missing", Some("new")).is_ok());
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn large_runs_dir_parses_once_across_repos_and_keeps_live() {
        let _g = crate::paths::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("cdamux");
        let repo_a = dir.path().join("repo-a");
        let repo_b = dir.path().join("repo-b");
        fs::create_dir_all(&repo_a).unwrap();
        fs::create_dir_all(&repo_b).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        let runs = home.join("runs");
        fs::create_dir_all(&runs).unwrap();
        let a = repo_a.display().to_string();
        let b = repo_b.display().to_string();
        // Hundreds of finished noise runs for other repos (shared CDAMUX_HOME).
        for i in 0..300 {
            write(
                &runs.join(format!("noise-{i}.json")),
                &format!(
                    r#"{{"run_id":"noise-{i}","repo":"{b}","status":"ok","ended_at":"t"}}"#
                ),
                100 + i as u64,
            );
        }
        write(
            &runs.join("a-live.json"),
            &format!(r#"{{"run_id":"a-live","repo":"{a}","status":"running"}}"#),
            2,
        );
        write(
            &runs.join("a-old.json"),
            &format!(r#"{{"run_id":"a-old","repo":"{a}","status":"ok","ended_at":"t"}}"#),
            50,
        );
        test_reset_run_file_parse_count();
        let live = list_live_orch_runs(&repo_a);
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].run_id, "a-live");
        let recent = list_orch_graph_runs(&repo_a, 5);
        assert_eq!(recent[0].run_id, "a-old");
        let after_a = test_run_file_parse_count();
        assert!(after_a >= 302, "expected full dir parse, got {after_a}");
        // Second repo must reuse the mtime index (no re-parse).
        let _ = list_live_orch_runs(&repo_b);
        let _ = list_orch_graph_runs(&repo_b, 5);
        assert_eq!(test_run_file_parse_count(), after_a);
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn finished_run_is_not_listed_as_live() {
        let _g = crate::paths::env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("cdamux");
        let repo = dir.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        let path = home.join("runs/legacy.json");
        write(
            &path,
            &format!(
                r#"{{"run_id":"legacy","repo":"{}","status":"running"}}"#,
                repo.display()
            ),
            1,
        );
        let runs = list_live_orch_runs(&repo);
        assert_eq!(runs.len(), 1);
        assert_eq!(
            (runs[0].run_id.as_str(), runs[0].session.as_deref()),
            ("legacy", None)
        );
        assert_eq!(find_orch_run(&repo, "legacy"), Some(path.clone()));
        write(
            &path,
            &format!(
                r#"{{"run_id":"legacy","repo":"{}","status":"ok","ended_at":"t"}}"#,
                repo.display()
            ),
            1,
        );
        assert!(list_live_orch_runs(&repo).is_empty());
        assert_eq!(list_orch_graph_runs(&repo, 5)[0].run_id, "legacy");
        std::env::remove_var("CDAMUX_HOME");
    }
}
