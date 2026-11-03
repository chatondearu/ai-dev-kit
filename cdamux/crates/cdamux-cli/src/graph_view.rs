//! Drawn orch graph: grid widget, sidecar / full-screen panel, node detail.

use crate::fleet_draw::{node_state_color, CLR_MUTED, CLR_SEL_BG, CLR_TEXT};
use cdamux_core::{
    find_orch_graph_from_cwd, format_orch_graph_panel_with_history, graph_node_order,
    layout_orch_graph, list_orch_graph_runs, orch_graph_header, orch_graph_is_stale,
    tmux_has_session, CellClass, GraphLayout, NodeState, OrchGraphError, OrchGraphFile,
    OrchGraphNode, BOX_H,
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
    Frame,
};
use std::path::Path;

/// Footer while the full-screen Graph view is open.
pub(crate) const GRAPH_FOOTER: &str = "j/k ↑↓ select node · Enter jump to session · g/Esc back · q quit";

/// Orch graph for the selected Fleet row: text tree (fallback) and parsed graph.
pub(crate) struct OrchGraphView {
    pub(crate) text: String,
    pub(crate) graph: Option<OrchGraphFile>,
    pub(crate) stale: bool,
    pub(crate) header: String,
}

impl OrchGraphView {
    fn text_only(text: String) -> Self {
        Self { text, graph: None, stale: false, header: String::new() }
    }

    /// Node at `selected` in navigation order (clamped).
    pub(crate) fn node_at(&self, selected: usize) -> Option<&OrchGraphNode> {
        let g = self.graph.as_ref()?;
        let order = graph_node_order(g);
        let i = *order.get(selected.min(order.len().saturating_sub(1)))?;
        g.nodes.get(i)
    }

    pub(crate) fn node_count(&self) -> usize {
        self.graph.as_ref().map_or(0, |g| g.nodes.len())
    }

    /// Redraw check for the refresh scheduler; `text` already renders every node.
    pub(crate) fn differs(&self, other: &Self) -> bool {
        self.text != other.text
            || self.header != other.header
            || self.stale != other.stale
            || self.graph.is_some() != other.graph.is_some()
    }
}

pub(crate) fn load_orch_graph_view(cwd: &str, width: usize) -> OrchGraphView {
    if cwd.is_empty() {
        return OrchGraphView::text_only("Select a project or session with a live orch run.".into());
    }
    match find_orch_graph_from_cwd(cwd) {
        Ok((g, age)) => OrchGraphView {
            text: format_orch_graph_panel_with_history(&g, width, age, Path::new(cwd)),
            stale: orch_graph_is_stale(&g, age),
            header: orch_graph_header(&g, age).join(" · "),
            graph: Some(g),
        },
        Err(OrchGraphError::NotFound(_)) => {
            let runs = list_orch_graph_runs(Path::new(cwd), 5);
            if runs.is_empty() {
                return OrchGraphView::text_only(
                    "(no live graph — start orch.sh in this repo; tab cycles with sidecar)".into(),
                );
            }
            let mut s = String::from("(no live graph)\n\nrecent runs:\n");
            for r in runs {
                let st = r.status.unwrap_or_else(|| "?".into());
                s.push_str(&format!("  {} · {}\n", r.run_id, st));
            }
            OrchGraphView::text_only(s)
        }
        Err(OrchGraphError::Invalid { path, message }) => OrchGraphView::text_only(format!(
            "invalid current.json at {}:\n{}",
            path.display(),
            message
        )),
    }
}

/// Session to jump to from a node. Today: a live tmux session named like the
/// node id. U1/U2 session links replace this lookup.
pub(crate) fn node_session(node: &OrchGraphNode) -> Option<String> {
    tmux_has_session(&node.id).ok()?.then(|| node.id.clone())
}

/// Renders a [`GraphLayout`]; scrolls vertically to keep `selected` (a box index) in view.
pub(crate) struct GraphWidget<'a> {
    pub(crate) layout: &'a GraphLayout,
    pub(crate) selected: Option<usize>,
}

fn scroll_top(box_y: usize, total: usize, view: usize) -> usize {
    if total <= view {
        return 0;
    }
    (box_y + BOX_H / 2).saturating_sub(view / 2).min(total - view)
}

impl Widget for GraphWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let sel = self.selected.and_then(|i| self.layout.boxes.get(i));
        let top = sel.map_or(0, |b| scroll_top(b.y, self.layout.height, area.height as usize));
        let rows = self.layout.cells.iter().enumerate().skip(top).take(area.height as usize);
        for (y, row) in rows {
            for (x, cell) in row.iter().enumerate().take(area.width as usize) {
                let mut style = match cell.class {
                    CellClass::Blank => continue,
                    CellClass::Edge => Style::default().fg(CLR_MUTED),
                    CellClass::Node(state) => Style::default().fg(node_state_color(state)),
                };
                let in_sel = sel.is_some_and(|b| x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + BOX_H);
                if in_sel {
                    style = style.bg(CLR_SEL_BG).add_modifier(Modifier::BOLD);
                }
                let pos = (area.x + x as u16, area.y + (y - top) as u16);
                if let Some(c) = buf.cell_mut(pos) {
                    c.set_char(cell.ch).set_style(style);
                }
            }
        }
    }
}

/// Header line + drawn DAG, or the text tree when there is no graph or it does not fit.
pub(crate) fn draw_orch_graph_panel(f: &mut Frame, area: Rect, view: &OrchGraphView, selected: Option<usize>) {
    let layout = view
        .graph
        .as_ref()
        .and_then(|g| layout_orch_graph(g, view.stale, area.width as usize));
    let Some(layout) = layout else {
        f.render_widget(
            Paragraph::new(view.text.as_str())
                .style(Style::default().fg(CLR_TEXT))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);
    f.render_widget(
        Paragraph::new(view.header.as_str()).style(Style::default().fg(CLR_MUTED)),
        parts[0],
    );
    f.render_widget(GraphWidget { layout: &layout, selected }, parts[1]);
}

pub(crate) fn node_detail_text(node: &OrchGraphNode, stale: bool) -> String {
    let state = NodeState::from_status(&node.status, stale);
    let mut lines = vec![format!(
        "{} {} · {} · status {} · phase {}",
        state.icon(),
        node.id,
        node.kind,
        node.status,
        node.phase
    )];
    if let Some(p) = node.parent.as_deref().filter(|p| !p.is_empty()) {
        lines.push(format!("parent {p}"));
    }
    if !node.message.is_empty() {
        lines.push(node.message.clone());
    }
    lines.join("\n")
}

/// Full-screen Graph view: the panel with a selection, and the node detail below.
pub(crate) fn draw_graph_screen(f: &mut Frame, area: Rect, view: &OrchGraphView, selected: usize) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(6)])
        .split(area);
    draw_orch_graph_panel(f, parts[0], view, Some(selected));
    let detail = view
        .node_at(selected)
        .map(|n| node_detail_text(n, view.stale))
        .unwrap_or_else(|| "(no node selected)".into());
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(CLR_MUTED))
        .title(" node ");
    f.render_widget(
        Paragraph::new(detail)
            .style(Style::default().fg(CLR_TEXT))
            .wrap(Wrap { trim: false })
            .block(block),
        parts[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet_draw::CLR_OK;

    fn buffer_lines(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> Vec<String> {
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    fn view() -> OrchGraphView {
        let raw = r#"{"run_id":"r1","status":"running","nodes":[
            {"id":"integrator","kind":"integrator","phase":"pending","status":"queued","message":"epic","parent":null},
            {"id":"U0","kind":"worker","phase":"implement","status":"running","message":"harness","parent":"integrator"},
            {"id":"U5","kind":"worker","phase":"implement","status":"done","message":"graph view","parent":"integrator"}
        ]}"#;
        let g: OrchGraphFile = serde_json::from_str(raw).unwrap();
        OrchGraphView { text: "tree".into(), header: "run r1".into(), stale: false, graph: Some(g) }
    }

    #[test]
    fn graph_screen_draws_boxes_selection_and_detail() {
        let v = view();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 16)).unwrap();
        // Navigation order: integrator, U0, U5 → select U0.
        term.draw(|f| draw_graph_screen(f, f.area(), &v, 1)).unwrap();
        let lines = buffer_lines(&term);
        let text = lines.join("\n");
        assert!(lines[0].starts_with("run r1"), "{text}");
        assert!(text.contains("◎ U0") && text.contains("✓ U5") && text.contains('▶'), "{text}");
        assert!(text.contains("U0 · worker · status running") && text.contains("harness"), "{text}");
        let y = lines.iter().position(|l| l.contains("◎ U0")).unwrap() as u16;
        let x = lines[y as usize].chars().position(|c| c == '◎').unwrap() as u16;
        let cell = &term.backend().buffer()[(x, y)];
        assert_eq!((cell.fg, cell.bg), (CLR_OK, CLR_SEL_BG));
    }

    #[test]
    fn narrow_panel_falls_back_to_text() {
        let v = view();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(14, 6)).unwrap();
        term.draw(|f| draw_orch_graph_panel(f, f.area(), &v, None)).unwrap();
        assert!(buffer_lines(&term)[0].starts_with("tree"));
    }

    #[test]
    fn scroll_keeps_selection_visible() {
        assert_eq!(scroll_top(40, 30, 50), 0);
        assert_eq!(scroll_top(0, 100, 20), 0);
        assert_eq!(scroll_top(50, 100, 20), 41);
        assert_eq!(scroll_top(96, 100, 20), 80);
    }
}
