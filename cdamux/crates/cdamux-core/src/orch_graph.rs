//! Live CDA orchestration graph (`.orch/graph/current.json`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Age after which a still-`running` graph is shown as stale in the TUI.
pub const ORCH_GRAPH_STALE_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphFile {
    pub run_id: String,
    #[serde(default)]
    pub repo: String,
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
    let path = orch_graph_path(repo);
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

fn is_stale(graph: &OrchGraphFile, age_secs: Option<u64>) -> bool {
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

fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// ASCII layout for ratatui side panel (dynamic snapshot).
pub fn format_orch_graph_panel(graph: &OrchGraphFile, width: usize) -> String {
    format_orch_graph_panel_ex(graph, width, None)
}

pub fn format_orch_graph_panel_ex(
    graph: &OrchGraphFile,
    width: usize,
    age_secs: Option<u64>,
) -> String {
    let w = width.clamp(40, 120);
    let mut lines = vec![format!("run {}", graph.run_id)];
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
            updated_line = format!("mtime {}", format_age(age));
        } else {
            updated_line.push_str(&format!(" · {}", format_age(age)));
        }
    }
    if is_stale(graph, age_secs) {
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
    let icon = match n.status.as_str() {
        "done" | "ok" | "skipped" => "✓",
        "failed" | "blocked" => "✗",
        "running" => "◎",
        _ => "·",
    };
    let head = format!(
        "{}{} {} [{}] {}",
        indent,
        icon,
        truncate(&n.id, 20),
        n.phase,
        truncate(&n.message, 24)
    );
    lines.push(truncate(&head, width));
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
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
            updated_at: "old".into(),
            nodes: vec![],
            edges: vec![],
            status: Some("running".into()),
            ended_at: None,
        };
        let out = format_orch_graph_panel_ex(&g, 60, Some(ORCH_GRAPH_STALE_SECS + 10));
        assert!(out.contains("stale"), "{out}");
    }
}
