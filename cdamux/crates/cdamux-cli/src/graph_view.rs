//! Drawn orch graph: grid widget, sidecar / full-screen panel, node detail.

use crate::fleet_draw::{node_state_color, CLR_MUTED, CLR_SEL_BG, CLR_TEXT};
use crate::fleet_tree::GraphTarget;
use cdamux_core::{
    find_orch_graph_from_cwd, find_orch_run_from_cwd, find_orch_run_status_from_cwd,
    format_orch_graph_panel_ex, format_orch_graph_panel_with_history, graph_node_order,
    layout_orch_graph, list_orch_graph_runs, load_orch_graph_at, orch_graph_header,
    orch_graph_is_stale, step_worker_id, tmux_has_session, worker_steps, CellClass, GraphLayout,
    NodeState, OrchGraphError, OrchGraphFile, OrchGraphNode, OrchWorkerStatus, Step, BOX_H,
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
    Frame,
};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Footer while the full-screen Graph view is open.
pub(crate) const GRAPH_FOOTER: &str =
    "j/k ↑↓ select node · Tab next run · Enter jump to session · g/Esc back · q quit";

/// Orch graph for the selected Fleet row: text tree (fallback) and parsed graph.
pub(crate) struct OrchGraphView {
    pub(crate) text: String,
    pub(crate) graph: Option<OrchGraphFile>,
    pub(crate) stale: bool,
    pub(crate) header: String,
    /// Workers from `.orch/run/status.json` next to the graph (step bar source).
    pub(crate) workers: Vec<OrchWorkerStatus>,
}

impl OrchGraphView {
    fn text_only(text: String) -> Self {
        Self { text, graph: None, stale: false, header: String::new(), workers: Vec::new() }
    }

    /// Node at `selected` in navigation order (clamped).
    pub(crate) fn node_at(&self, selected: usize) -> Option<&OrchGraphNode> {
        let g = self.graph.as_ref()?;
        let order = graph_node_order(g);
        let i = *order.get(selected.min(order.len().saturating_sub(1)))?;
        g.nodes.get(i)
    }

    /// Pipeline steps of the worker the node belongs to, when status.json knows it.
    pub(crate) fn steps_for(&self, node: &OrchGraphNode) -> Option<Vec<Step>> {
        let wid = step_worker_id(node)?;
        let worker = self.workers.iter().find(|w| w.id == wid)?;
        let parent = format!("worker-{wid}");
        let verifier = self.graph.as_ref()?.nodes.iter().find(|n| {
            n.kind == "verifier" && n.parent.as_deref() == Some(parent.as_str())
        });
        Some(worker_steps(worker, verifier.map(|n| NodeState::from_status(&n.status, self.stale)), now_secs()))
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

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Vertical step list for a Worker row's sidecar (its width is unknown here, so no bar).
pub(crate) fn worker_steps_text(worker: Option<&OrchWorkerStatus>) -> String {
    worker
        .map(|w| worker_steps(w, None, now_secs()).iter().map(Step::text).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// Graph for `target`; `pick` (Graph view `Tab`) cycles a session's runs, newest first.
pub(crate) fn load_orch_graph_view(target: &GraphTarget, pick: usize, width: usize) -> OrchGraphView {
    match target {
        GraphTarget::Latest(cwd) if !cwd.is_empty() => load_latest_view(cwd, width),
        GraphTarget::Runs { cwd, runs } if !runs.is_empty() => {
            let run = &runs[pick % runs.len()];
            let Some(path) = find_orch_run_from_cwd(cwd, run) else {
                return OrchGraphView::text_only(format!("(run {run} not found under .orch/graph)"));
            };
            match load_orch_graph_at(&path) {
                Ok((g, age)) => {
                    let mut header = orch_graph_header(&g, age).join(" · ");
                    if runs.len() > 1 {
                        header.push_str(&format!(" · {}/{} (Tab next run)", pick % runs.len() + 1, runs.len()));
                    }
                    OrchGraphView {
                        text: format_orch_graph_panel_ex(&g, width, age),
                        stale: orch_graph_is_stale(&g, age),
                        header,
                        // status.json describes one run; pre-#178 files carry no run id.
                        workers: find_orch_run_status_from_cwd(cwd)
                            .filter(|st| st.run_id.as_deref().is_none_or(|id| id == g.run_id))
                            .map(|st| st.workers)
                            .unwrap_or_default(),
                        graph: Some(g),
                    }
                }
                Err(e) => OrchGraphView::text_only(e.to_string()),
            }
        }
        GraphTarget::NoRun => OrchGraphView::text_only(
            "(no orch run started from this session)\n\nRuns nest under the session that started them; the project row shows the latest run.".into(),
        ),
        _ => OrchGraphView::text_only("Select a project or session with a live orch run.".into()),
    }
}

fn load_latest_view(cwd: &str, width: usize) -> OrchGraphView {
    match find_orch_graph_from_cwd(cwd) {
        Ok((g, age)) => OrchGraphView {
            text: format_orch_graph_panel_with_history(&g, width, age, Path::new(cwd)),
            stale: orch_graph_is_stale(&g, age),
            header: orch_graph_header(&g, age).join(" · "),
            workers: find_orch_run_status_from_cwd(cwd).map(|s| s.workers).unwrap_or_default(),
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

/// Step bar: one line when it fits `width`, else a vertical list. Colours follow the node map.
pub(crate) fn step_bar_lines(steps: &[Step], width: usize) -> Vec<Line<'static>> {
    let span = |s: &Step| Span::styled(s.text(), Style::default().fg(node_state_color(s.state)));
    let one_line = steps.iter().map(|s| s.text().chars().count()).sum::<usize>()
        + steps.len().saturating_sub(1) * 3;
    if one_line <= width {
        let mut spans = Vec::new();
        for (i, s) in steps.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" → ", Style::default().fg(CLR_MUTED)));
            }
            spans.push(span(s));
        }
        return vec![Line::from(spans)];
    }
    steps.iter().map(|s| Line::from(span(s))).collect()
}

/// Full-screen Graph view: the panel with a selection, the node detail, and the
/// selected worker's step bar under it.
pub(crate) fn draw_graph_screen(f: &mut Frame, area: Rect, view: &OrchGraphView, selected: usize) {
    let node = view.node_at(selected);
    let bar = node
        .and_then(|n| view.steps_for(n))
        .map(|steps| step_bar_lines(&steps, area.width as usize))
        .unwrap_or_default();
    let bar_h = (bar.len() as u16).min(area.height.saturating_sub(10));
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(6), Constraint::Length(bar_h)])
        .split(area);
    draw_orch_graph_panel(f, parts[0], view, Some(selected));
    let detail = node
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
    f.render_widget(Paragraph::new(bar), parts[2]);
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
        OrchGraphView { text: "tree".into(), header: "run r1".into(), stale: false, graph: Some(g), workers: vec![] }
    }

    /// `view()` plus a `worker-162` node (and its verifier child) tied to a status.json worker.
    fn steps_view() -> OrchGraphView {
        let raw = r#"{"run_id":"r1","status":"running","nodes":[
            {"id":"worker-162","kind":"worker","phase":"verify","status":"running","message":"w","parent":null},
            {"id":"worker-162-grounding","kind":"verifier","phase":"grounding","status":"done","message":"ok","parent":"worker-162"}
        ]}"#;
        let workers = r#"{"id":"162","phase":"ci","message":"PR #9","updated_at":"t","phases":[
            {"phase":"agent","started_at":100},{"phase":"verify","started_at":220},{"phase":"ci","started_at":230}]}"#;
        OrchGraphView {
            text: "tree".into(),
            header: "run r1".into(),
            stale: false,
            graph: serde_json::from_str(raw).ok(),
            workers: vec![serde_json::from_str(workers).unwrap()],
        }
    }

    #[test]
    fn step_bar_is_one_line_when_wide_and_a_list_when_narrow() {
        let v = steps_view();
        let node = v.node_at(0).unwrap();
        let mut wide = ratatui::Terminal::new(ratatui::backend::TestBackend::new(90, 16)).unwrap();
        wide.draw(|f| draw_graph_screen(f, f.area(), &v, 0)).unwrap();
        let lines = buffer_lines(&wide);
        let bar = lines.iter().find(|l| l.contains("implement")).expect("bar drawn");
        assert!(bar.contains("✓ implement 2m00s → ✓ verify 10s → ✓ verifier → ✓ review"), "{bar}");
        assert!(bar.contains("◎ CI") && bar.ends_with(' '), "{bar}");
        assert_eq!(lines.iter().filter(|l| l.contains("implement")).count(), 1);

        let mut narrow = ratatui::Terminal::new(ratatui::backend::TestBackend::new(24, 24)).unwrap();
        narrow.draw(|f| draw_graph_screen(f, f.area(), &v, 0)).unwrap();
        let text = buffer_lines(&narrow).join("\n");
        assert!(text.contains("✓ implement") && text.contains("\n✓ verify") && text.contains("· PR"), "{text}");
        assert_eq!(step_bar_lines(&v.steps_for(node).unwrap(), 24).len(), 6);
    }

    #[test]
    fn worker_sidecar_lists_steps() {
        let w = steps_view().workers.remove(0);
        let text = worker_steps_text(Some(&w));
        assert!(text.starts_with("✓ implement 2m00s\n✓ verify 10s\n✓ review"), "{text}");
        assert!(text.contains("\n◎ CI ") && text.ends_with("· PR"), "{text}");
        assert_eq!(worker_steps_text(None), "");
    }

    #[test]
    fn node_without_worker_has_no_step_bar() {
        let v = view(); // U0/U5 are not `worker-<id>` nodes
        assert!(v.steps_for(v.node_at(1).unwrap()).is_none());
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
