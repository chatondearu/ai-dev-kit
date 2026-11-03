//! Layered DAG layout of the orch graph on a character grid (no ratatui).
//!
//! Layers run left → right (longest path over `edges` + `parent`), nodes are
//! 3-row boxes, long edges pass through dummy slots, and edges between two
//! layers share one vertical lane drawn with box-drawing junctions.

use crate::orch_graph::OrchGraphFile;
use std::collections::HashMap;

/// Columns between two layers: out stub, lane, in stub, arrow.
const GAP: usize = 4;
pub const BOX_H: usize = 3;
const PITCH: usize = BOX_H + 1;
const MIN_BOX_W: usize = 8;
const MAX_BOX_W: usize = 22;

/// Display state of a node; one colour map in the CLI covers these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Queued,
    Running,
    Done,
    Error,
    Blocked,
    Stale,
}

impl NodeState {
    /// `graph_stale`: the whole run is past the 30-min badge, so `running` is stale.
    pub fn from_status(status: &str, graph_stale: bool) -> Self {
        match status {
            "done" | "ok" | "skipped" => Self::Done,
            "failed" | "error" => Self::Error,
            "blocked" => Self::Blocked,
            "running" if graph_stale => Self::Stale,
            "running" => Self::Running,
            _ => Self::Queued,
        }
    }

    pub fn icon(self) -> char {
        match self {
            Self::Queued => '·',
            Self::Running => '◎',
            Self::Done => '✓',
            Self::Error => '✗',
            Self::Blocked => '⊘',
            Self::Stale => '◌',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellClass {
    Blank,
    Edge,
    Node(NodeState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphCell {
    pub ch: char,
    pub class: CellClass,
}

/// One drawn node: `node` indexes `OrchGraphFile::nodes`; height is [`BOX_H`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeBox {
    pub node: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
}

#[derive(Debug, Clone)]
pub struct GraphLayout {
    pub width: usize,
    pub height: usize,
    /// `cells[y][x]`.
    pub cells: Vec<Vec<GraphCell>>,
    /// Navigation order: layer by layer, top to bottom.
    pub boxes: Vec<NodeBox>,
}

impl GraphLayout {
    pub fn row_string(&self, y: usize) -> String {
        self.cells[y].iter().map(|c| c.ch).collect()
    }
}

#[derive(Debug, Clone, Copy)]
enum Item {
    Node(usize),
    /// A long edge passing through this layer (index into the edge list).
    Dummy(usize),
}

struct Layered {
    /// `layers[l]` = items in display order.
    layers: Vec<Vec<Item>>,
    /// Segments between adjacent layers: ((layer, slot), (layer + 1, slot)).
    segs: Vec<((usize, usize), (usize, usize))>,
}

/// Forward edges as node indices: `edges` first, then `parent → child` for
/// nodes with no incoming edge (the parent link is implied by the dependency).
fn node_edges(graph: &OrchGraphFile) -> Vec<(usize, usize)> {
    let mut index = HashMap::new();
    for (i, n) in graph.nodes.iter().enumerate() {
        index.entry(n.id.as_str()).or_insert(i);
    }
    let mut out: Vec<(usize, usize)> = Vec::new();
    let push = |a: usize, b: usize, out: &mut Vec<(usize, usize)>| {
        if a != b && !out.contains(&(a, b)) {
            out.push((a, b));
        }
    };
    for e in &graph.edges {
        if let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) {
            push(a, b, &mut out);
        }
    }
    for (i, n) in graph.nodes.iter().enumerate() {
        let Some(&p) = n.parent.as_deref().and_then(|p| index.get(p)) else {
            continue;
        };
        if !out.iter().any(|&(_, b)| b == i) {
            push(p, i, &mut out);
        }
    }
    out
}

fn build_layers(graph: &OrchGraphFile) -> Layered {
    let n = graph.nodes.len();
    let edges = node_edges(graph);
    // Longest path; the `< n` cap ends relaxation on cycles.
    let mut layer = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for &(a, b) in &edges {
            if layer[b] < layer[a] + 1 && layer[a] + 1 < n {
                layer[b] = layer[a] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let depth = layer.iter().max().map_or(0, |m| m + 1);

    // Items per layer (unordered yet) and item-level segments.
    let mut items: Vec<Vec<Item>> = vec![Vec::new(); depth];
    let mut item_at = vec![(0usize, 0usize); n];
    for i in 0..n {
        item_at[i] = (layer[i], items[layer[i]].len());
        items[layer[i]].push(Item::Node(i));
    }
    let mut item_segs = Vec::new();
    for (ei, &(a, b)) in edges.iter().enumerate() {
        // shortcut: back edges of a cycle are not drawn; the detail panel still lists nodes.
        if layer[b] <= layer[a] {
            continue;
        }
        let mut prev = item_at[a];
        for (l, slot) in items.iter_mut().enumerate().take(layer[b]).skip(layer[a] + 1) {
            let cur = (l, slot.len());
            slot.push(Item::Dummy(ei));
            item_segs.push((prev, cur));
            prev = cur;
        }
        item_segs.push((prev, item_at[b]));
    }

    // Order: layer 0 by file order; later layers by the topmost predecessor
    // slot, then file order (dummies after nodes) — stable and fan-out friendly.
    let mut slot_of: Vec<Vec<usize>> = items.iter().map(|l| (0..l.len()).collect()).collect();
    let mut layers = Vec::with_capacity(depth);
    for l in 0..depth {
        let mut order: Vec<usize> = (0..items[l].len()).collect();
        let key = |k: usize| {
            let pred = item_segs
                .iter()
                .filter(|(_, to)| *to == (l, k))
                .map(|(from, _)| slot_of[from.0][from.1])
                .min()
                .unwrap_or(usize::MAX);
            let tie = match items[l][k] {
                Item::Node(i) => i,
                Item::Dummy(e) => n + e,
            };
            (pred, tie)
        };
        order.sort_by_cached_key(|&k| key(k));
        for (slot, &k) in order.iter().enumerate() {
            slot_of[l][k] = slot;
        }
        layers.push(order.iter().map(|&k| items[l][k]).collect());
    }
    let segs = item_segs
        .iter()
        .map(|(a, b)| ((a.0, slot_of[a.0][a.1]), (b.0, slot_of[b.0][b.1])))
        .collect();
    Layered { layers, segs }
}

/// Node indices in navigation order (same as [`GraphLayout::boxes`]).
pub fn graph_node_order(graph: &OrchGraphFile) -> Vec<usize> {
    build_layers(graph)
        .layers
        .iter()
        .flatten()
        .filter_map(|it| match it {
            Item::Node(i) => Some(*i),
            Item::Dummy(_) => None,
        })
        .collect()
}

fn clean(s: &str) -> impl Iterator<Item = char> + '_ {
    // Untrusted JSON: never put control characters into the terminal grid.
    s.chars().map(|c| if c.is_control() { '?' } else { c })
}

fn fit(s: &str, max: usize) -> Vec<char> {
    let chars: Vec<char> = clean(s).collect();
    if chars.len() <= max {
        return chars;
    }
    let mut out: Vec<char> = chars.into_iter().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

const L: u8 = 1;
const R: u8 = 2;
const U: u8 = 4;
const D: u8 = 8;

fn junction(mask: u8) -> char {
    match mask {
        m if m == U | D | L | R => '┼',
        m if m == U | D | R => '├',
        m if m == U | D | L => '┤',
        m if m == L | R | D => '┬',
        m if m == L | R | U => '┴',
        m if m == R | D => '┌',
        m if m == L | D => '┐',
        m if m == U | R => '└',
        m if m == U | L => '┘',
        m if m & (U | D) != 0 && m & (L | R) == 0 => '│',
        _ => '─',
    }
}

/// Lay out `graph` to fit `width` columns. `None` when there is nothing to
/// draw or the layers do not fit: the caller falls back to the text tree.
/// Height is unbounded; the widget scrolls to keep the selection visible.
pub fn layout_orch_graph(graph: &OrchGraphFile, graph_stale: bool, width: usize) -> Option<GraphLayout> {
    if graph.nodes.is_empty() {
        return None;
    }
    let Layered { layers, segs } = build_layers(graph);
    let depth = layers.len();
    let gaps = (depth - 1) * GAP;
    let want = graph
        .nodes
        .iter()
        .map(|nd| nd.id.chars().count().max(nd.kind.chars().count()) + 4)
        .max()
        .unwrap_or(MIN_BOX_W)
        .clamp(MIN_BOX_W, MAX_BOX_W);
    let bw = want.min(width.saturating_sub(gaps) / depth);
    if bw < MIN_BOX_W {
        return None;
    }
    let max_slots = layers.iter().map(Vec::len).max().unwrap_or(1);
    let height = max_slots * PITCH - 1;
    let total_w = depth * bw + gaps;
    let col_x = |l: usize| l * (bw + GAP);
    let slot_y = |l: usize, s: usize| (max_slots - layers[l].len()) * PITCH / 2 + s * PITCH;

    let blank = GraphCell { ch: ' ', class: CellClass::Blank };
    let mut cells = vec![vec![blank; total_w]; height];
    let mut boxes = Vec::new();
    for (l, layer) in layers.iter().enumerate() {
        for (s, item) in layer.iter().enumerate() {
            let (x, y) = (col_x(l), slot_y(l, s));
            match *item {
                Item::Dummy(_) => {
                    for c in &mut cells[y + 1][x..x + bw] {
                        *c = GraphCell { ch: '─', class: CellClass::Edge };
                    }
                }
                Item::Node(i) => {
                    let nd = &graph.nodes[i];
                    let class = CellClass::Node(NodeState::from_status(&nd.status, graph_stale));
                    let inner = bw - 2;
                    let mut top = vec!['─'; inner];
                    for (k, ch) in fit(&nd.kind, inner).into_iter().enumerate() {
                        top[k] = ch;
                    }
                    let mut mid = vec![' '; inner];
                    mid[0] = NodeState::from_status(&nd.status, graph_stale).icon();
                    for (k, ch) in fit(&nd.id, inner - 2).into_iter().enumerate() {
                        mid[k + 2] = ch;
                    }
                    let rows = [
                        ('╭', top, '╮'),
                        ('│', mid, '│'),
                        ('╰', vec!['─'; inner], '╯'),
                    ];
                    for (dy, (left, body, right)) in rows.into_iter().enumerate() {
                        let row = &mut cells[y + dy];
                        let chars = std::iter::once(left).chain(body).chain(std::iter::once(right));
                        for (k, ch) in chars.enumerate() {
                            row[x + k] = GraphCell { ch, class };
                        }
                    }
                    boxes.push(NodeBox { node: i, x, y, w: bw });
                }
            }
        }
    }

    // Edge masks in the gaps, then junction glyphs and arrowheads.
    // shortcut: one lane per gap, so two sources feeding two targets read as a
    // full bipartite join; give each source its own lane if that misleads.
    let mut mask = vec![vec![0u8; total_w]; height];
    let mut arrows = Vec::new();
    for &((la, sa), (lb, sb)) in &segs {
        let ya = slot_y(la, sa) + 1;
        let yb = slot_y(lb, sb) + 1;
        let xs = col_x(la) + bw;
        let lane = xs + GAP / 2;
        let xe = xs + GAP - 1;
        mask[ya][xs] |= L;
        for x in xs..lane {
            mask[ya][x] |= R;
            mask[ya][x + 1] |= L;
        }
        for y in ya.min(yb)..ya.max(yb) {
            mask[y][lane] |= D;
            mask[y + 1][lane] |= U;
        }
        for x in lane..xe {
            mask[yb][x] |= R;
            mask[yb][x + 1] |= L;
        }
        mask[yb][xe] |= R;
        if matches!(layers[lb][sb], Item::Node(_)) {
            arrows.push((xe, yb));
        }
    }
    for (y, row) in mask.iter().enumerate() {
        for (x, &m) in row.iter().enumerate() {
            if m != 0 {
                cells[y][x] = GraphCell { ch: junction(m), class: CellClass::Edge };
            }
        }
    }
    for (x, y) in arrows {
        cells[y][x].ch = '▶';
    }

    Some(GraphLayout { width: total_w, height, cells, boxes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orch_graph::{OrchGraphEdge, OrchGraphNode};

    fn node(id: &str, status: &str, parent: Option<&str>) -> OrchGraphNode {
        OrchGraphNode {
            id: id.into(),
            kind: "worker".into(),
            phase: "implement".into(),
            status: status.into(),
            message: String::new(),
            parent: parent.map(Into::into),
        }
    }

    fn graph(nodes: Vec<OrchGraphNode>, edges: &[(&str, &str)]) -> OrchGraphFile {
        OrchGraphFile {
            run_id: "r".into(),
            repo: String::new(),
            session: None,
            updated_at: String::new(),
            nodes,
            edges: edges
                .iter()
                .map(|(a, b)| OrchGraphEdge { from: (*a).into(), to: (*b).into(), edge_type: "dep".into() })
                .collect(),
            status: Some("running".into()),
            ended_at: None,
        }
    }

    fn ids(g: &OrchGraphFile, lay: &GraphLayout) -> Vec<(String, usize, usize)> {
        lay.boxes.iter().map(|b| (g.nodes[b.node].id.clone(), b.x, b.y)).collect()
    }

    fn dump(lay: &GraphLayout) -> String {
        (0..lay.height).map(|y| lay.row_string(y)).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn chain_is_one_row_of_layers() {
        let g = graph(
            vec![node("a", "done", None), node("b", "running", None), node("c", "queued", None)],
            &[("a", "b"), ("b", "c")],
        );
        let lay = layout_orch_graph(&g, false, 80).unwrap();
        let pos = ids(&g, &lay);
        assert_eq!(pos.iter().map(|p| p.2).collect::<Vec<_>>(), vec![0, 0, 0]);
        assert!(pos[0].1 < pos[1].1 && pos[1].1 < pos[2].1);
        assert_eq!(lay.height, BOX_H);
        let mid = lay.row_string(1);
        assert_eq!(mid.matches('▶').count(), 2, "{}", dump(&lay));
        assert!(mid.contains("✓ a") && mid.contains("◎ b") && mid.contains("· c"), "{mid}");
        assert_eq!(lay.cells[1][pos[1].1].class, CellClass::Node(NodeState::Running));
    }

    #[test]
    fn fan_out_from_parent_uses_one_lane() {
        let g = graph(
            vec![
                node("integrator", "queued", None),
                node("U0", "running", Some("integrator")),
                node("U5", "running", Some("integrator")),
                node("U7", "running", Some("integrator")),
            ],
            &[],
        );
        let lay = layout_orch_graph(&g, false, 80).unwrap();
        let pos = ids(&g, &lay);
        assert_eq!(pos[0].0, "integrator");
        // Workers stack in file order; the integrator sits beside the middle one.
        assert_eq!(pos.iter().skip(1).map(|p| p.2).collect::<Vec<_>>(), vec![0, 4, 8]);
        assert_eq!(pos[0].2, 4);
        let text = dump(&lay);
        for ch in ['┌', '┼', '└'] {
            assert!(text.contains(ch), "missing {ch}\n{text}");
        }
        assert_eq!(text.matches('▶').count(), 3, "{text}");
    }

    #[test]
    fn fan_in_merges_into_one_arrow() {
        let g = graph(
            vec![node("a", "done", None), node("b", "done", None), node("c", "queued", None)],
            &[("a", "c"), ("b", "c")],
        );
        let lay = layout_orch_graph(&g, false, 80).unwrap();
        let text = dump(&lay);
        assert_eq!(text.matches('▶').count(), 1, "{text}");
        assert!(text.contains('┐') && text.contains('┘'), "{text}");
    }

    #[test]
    fn diamond_with_skip_edge_routes_through_dummy() {
        let g = graph(
            vec![
                node("a", "done", None),
                node("b", "running", None),
                node("c", "blocked", None),
                node("d", "queued", None),
            ],
            &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d"), ("a", "d")],
        );
        let lay = layout_orch_graph(&g, false, 80).unwrap();
        let pos = ids(&g, &lay);
        assert_eq!(pos.len(), 4);
        assert_eq!(pos[1].1, pos[2].1, "b and c share layer 1");
        assert!(pos[3].1 > pos[1].1);
        // Layer 1 holds b, c and the dummy for a → d: three slots.
        assert_eq!(lay.height, 3 * PITCH - 1, "{}", dump(&lay));
        let dummy_row = (0..lay.height)
            .find(|&y| lay.cells[y][pos[1].1].ch == '─' && lay.cells[y][pos[1].1].class == CellClass::Edge);
        assert!(dummy_row.is_some(), "{}", dump(&lay));
        // b, c, and one shared arrow into d for b → d, c → d and a → d.
        assert_eq!(dump(&lay).matches('▶').count(), 3, "{}", dump(&lay));
    }

    #[test]
    fn twenty_five_nodes_do_not_overlap() {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for l in 0..5 {
            for s in 0..5 {
                nodes.push(node(&format!("n{l}{s}"), "queued", None));
                if l > 0 {
                    edges.push((format!("n{}{}", l - 1, s), format!("n{l}{s}")));
                    edges.push((format!("n{}{}", l - 1, (s + 1) % 5), format!("n{l}{s}")));
                }
            }
        }
        let e: Vec<(&str, &str)> = edges.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let g = graph(nodes, &e);
        let lay = layout_orch_graph(&g, false, 120).unwrap();
        assert_eq!(lay.boxes.len(), 25);
        assert!(lay.width <= 120);
        for (i, a) in lay.boxes.iter().enumerate() {
            for b in &lay.boxes[i + 1..] {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + BOX_H <= b.y || b.y + BOX_H <= a.y;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
        assert_eq!(graph_node_order(&g), lay.boxes.iter().map(|b| b.node).collect::<Vec<_>>());
    }

    #[test]
    fn too_narrow_falls_back() {
        let g = graph(
            vec![node("a", "done", None), node("b", "running", None), node("c", "queued", None)],
            &[("a", "b"), ("b", "c")],
        );
        assert!(layout_orch_graph(&g, false, 20).is_none());
        assert!(layout_orch_graph(&graph(vec![], &[]), false, 80).is_none());
    }

    #[test]
    fn cycle_terminates_and_parent_yields_to_edges() {
        let g = graph(
            vec![node("a", "done", None), node("b", "done", Some("a")), node("c", "running", Some("a"))],
            &[("b", "c"), ("c", "b")],
        );
        let lay = layout_orch_graph(&g, false, 120).unwrap();
        assert_eq!(lay.boxes.len(), 3);
        // b and c both have incoming edges, so a has no drawn children.
        assert_eq!(lay.boxes[0].node, 0);
    }

    #[test]
    fn states_and_stale_running() {
        assert_eq!(NodeState::from_status("running", true), NodeState::Stale);
        assert_eq!(NodeState::from_status("failed", false), NodeState::Error);
        assert_eq!(NodeState::from_status("pending", false), NodeState::Queued);
        let g = graph(vec![node("x\u{1b}[31m", "running", None)], &[]);
        let lay = layout_orch_graph(&g, true, 40).unwrap();
        assert!(lay.row_string(1).contains("◌ x?[31m"), "{}", lay.row_string(1));
    }
}
