//! Live CDA orchestration graph (`.orch/graph/current.json`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphFile {
    pub run_id: String,
    pub repo: String,
    pub updated_at: String,
    pub nodes: Vec<OrchGraphNode>,
    pub edges: Vec<OrchGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphNode {
    pub id: String,
    pub kind: String,
    pub phase: String,
    pub status: String,
    pub message: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchGraphEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

pub fn orch_graph_path(repo: &Path) -> PathBuf {
    repo.join(".orch/graph/current.json")
}

pub fn load_orch_graph(repo: &Path) -> Option<OrchGraphFile> {
    let path = orch_graph_path(repo);
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Walk parents from `cwd` until `.orch/graph/current.json` is found.
pub fn find_orch_graph_from_cwd(cwd: &str) -> Option<OrchGraphFile> {
    if cwd.is_empty() {
        return None;
    }
    let mut cur = Path::new(cwd);
    while let Some(parent) = cur.parent() {
        if let Some(g) = load_orch_graph(cur) {
            return Some(g);
        }
        if parent == cur {
            break;
        }
        cur = parent;
    }
    None
}

/// ASCII layout for ratatui side panel (dynamic snapshot).
pub fn format_orch_graph_panel(graph: &OrchGraphFile, width: usize) -> String {
    let w = width.clamp(40, 120);
    let mut lines = vec![
        format!("run {}", graph.run_id),
        format!("updated {}", graph.updated_at),
        String::new(),
    ];

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
        "done" => "✓",
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
        };
        let out = format_orch_graph_panel(&g, 60);
        assert!(out.contains("run r1"));
        assert!(out.contains("integrator"));
    }
}
