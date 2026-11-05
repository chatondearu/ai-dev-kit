//! Drawing code for the fleet dashboard: tabs, tree column, sidecar and overlays.

use crate::app_tabs::{
    draw_agents, draw_projects, draw_sessions_list, draw_tab_bar, help_for_tab, AppTab,
};
use crate::fleet_tree::TreeRow;
use crate::graph_view::{draw_graph_screen, draw_orch_graph_panel, OrchGraphView, GRAPH_FOOTER};
use crate::keymap::{fit_footer, fleet_help_groups, fleet_help_text, HELP_LEFT, HELP_RIGHT};
use crate::settings_form::{draw_settings, SettingsForm};
use crate::widgets::{draw_scrollbar, draw_text_modal, text_field, text_modal_inner_width};
use ansi_to_tui::IntoText as _;
use cdamux_core::{
    state_icon, ChildKind, Config, EnrichedSession, GroupBy, NodeState, SessionChrome,
    SessionState,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub(crate) const COMPOSER_LABEL: &str = "send";

/// Palette aligned with PRD-41 (archived HTML mockup: `foam/archive/cdamux-prototype/`).
pub(crate) const CLR_TEXT: Color = Color::Rgb(230, 234, 240);
pub(crate) const CLR_MUTED: Color = Color::Rgb(139, 149, 168);
pub(crate) const CLR_ACCENT: Color = Color::Rgb(110, 181, 255);
pub(crate) const CLR_PROJECT: Color = Color::Rgb(158, 200, 255);
pub(crate) const CLR_OK: Color = Color::Rgb(107, 207, 142);
pub(crate) const CLR_WARN: Color = Color::Rgb(232, 184, 74);
pub(crate) const CLR_IDLE: Color = Color::Rgb(92, 101, 120);
pub(crate) const CLR_SEL_BG: Color = Color::Rgb(42, 49, 64);
pub(crate) const CLR_ERR: Color = Color::Rgb(240, 113, 120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusArea {
    Tree,
    Composer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideTab {
    Preview,
    Notes,
    Review,
    OrchGraph,
}

impl SideTab {
    pub(crate) fn next(self) -> Self {
        match self {
            SideTab::Preview => SideTab::Notes,
            SideTab::Notes => SideTab::Review,
            SideTab::Review => SideTab::OrchGraph,
            SideTab::OrchGraph => SideTab::Preview,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            SideTab::Preview => "Preview",
            SideTab::Notes => "Notes",
            SideTab::Review => "Review",
            SideTab::OrchGraph => "Orch graph",
        }
    }
}

pub(crate) struct RenameState {
    pub(crate) old_name: String,
    pub(crate) input: String,
}

/// Rename uses the shared 5-row text modal so its hint line survives short terminals.
pub(crate) fn draw_rename_modal(f: &mut Frame, state: &RenameState) {
    let inner_w = text_modal_inner_width(f.area());
    let body = Text::from(vec![
        Line::from(format!("from: {}", state.old_name)),
        Line::from(vec![
            Span::raw("new name: "),
            text_field(&state.input, true, inner_w.saturating_sub(10)),
        ]),
        Line::from("Enter save · Esc cancel"),
    ]);
    draw_text_modal(f, f.area(), " Rename session ".into(), body);
}

/// `scroll_from_bottom`: 0 = tail of capture; higher = scroll up into older lines.
/// `viewport_lines`: visible rows in the sidecar (show the newest lines that fit).
pub(crate) fn preview_display_lines(
    raw: &str,
    scroll_from_bottom: usize,
    viewport_lines: u16,
) -> Vec<Line<'_>> {
    let text: Text = raw
        .into_text()
        .unwrap_or_else(|_| Text::raw(raw));
    let window = viewport_lines.max(1) as usize;
    let end = text.lines.len().saturating_sub(scroll_from_bottom);
    let start = end.saturating_sub(window);
    if start >= end {
        return Vec::new();
    }
    text.lines[start..end].to_vec()
}

/// Preview scroll: `delta` > 0 goes up into older lines; clamped so the top of
/// the capture is the furthest you can go (`total` lines, `view` visible rows).
pub(crate) fn scroll_preview(cur: usize, delta: isize, total: usize, view: u16) -> usize {
    cur.saturating_add_signed(delta).min(total.saturating_sub(view as usize))
}

pub(crate) fn sidecar_plain_body(tab: SideTab, notes: &str, review: &str, orch_graph: &str) -> String {
    match tab {
        SideTab::Preview => String::new(),
        SideTab::Notes => notes.to_string(),
        SideTab::Review => review.to_string(),
        SideTab::OrchGraph => orch_graph.to_string(),
    }
}

pub(crate) fn review_sidecar_hint(session: Option<&EnrichedSession>) -> String {
    match session {
        None => "Select a session.\n\nCtrl+R or Enter: open hunk review TUI.".to_string(),
        Some(s) => format!(
            "Session: {}\nRepo: {}\n\nCtrl+R or Enter: open hunk review (git diff → send to session).",
            s.name,
            s.path
        ),
    }
}

pub(crate) struct DrawState<'a> {
    pub(crate) rows: &'a [TreeRow],
    pub(crate) selected_row: usize,
    pub(crate) side_tab: SideTab,
    pub(crate) focus: FocusArea,
    pub(crate) preview_text: &'a str,
    pub(crate) preview_scroll: usize,
    pub(crate) notes_text: &'a str,
    pub(crate) review_text: &'a str,
    pub(crate) orch_graph: &'a OrchGraphView,
    /// Full-screen Graph view open, with the selected node (navigation order).
    pub(crate) graph_view: Option<usize>,
    pub(crate) composer_line: &'a str,
    pub(crate) composer_session: Option<&'a str>,
    pub(crate) session_chrome: Option<&'a SessionChrome>,
    pub(crate) status_line: &'a str,
}

/// One state → colour map for orch graph nodes; Fleet sessions map onto it.
pub(crate) fn node_state_color(state: NodeState) -> Color {
    match state {
        NodeState::Queued => CLR_IDLE,
        NodeState::Running => CLR_OK,
        NodeState::Done => CLR_ACCENT,
        NodeState::Error => CLR_ERR,
        NodeState::Blocked => CLR_WARN,
        NodeState::Stale => CLR_MUTED,
    }
}

pub(crate) fn session_state_color(state: SessionState) -> Color {
    node_state_color(match state {
        SessionState::Idle => NodeState::Queued,
        SessionState::Active => NodeState::Running,
        SessionState::NeedsInput => NodeState::Blocked,
        SessionState::Done => NodeState::Done,
        SessionState::Error => NodeState::Error,
    })
}

/// Below this width the sidecar stacks under the tree instead of beside it.
pub(crate) const STACK_BELOW_COLS: u16 = 100;
/// Session chrome box: two info lines inside borders.
const CHROME_H: u16 = 4;
/// Sidecar rows needed to show it (tabs 1 + chrome 4 + body 4 + composer 2).
const SIDECAR_CHROME_MIN_H: u16 = 11;

fn tree_list_block(stacked: bool) -> Block<'static> {
    // RIGHT always: scrollbar paints the right border column; BOTTOM separates
    // the tree from the sidecar when stacked (width < STACK_BELOW_COLS).
    Block::default()
        .borders(if stacked {
            Borders::BOTTOM | Borders::RIGHT
        } else {
            Borders::RIGHT
        })
        .border_style(Style::default().fg(CLR_MUTED))
        .title(Span::styled(
            " projects → sessions ",
            Style::default().fg(CLR_ACCENT).add_modifier(Modifier::BOLD),
        ))
}

/// `inner`: the tree list rows (inside its block), as stored by [`draw_fleet_body`].
pub(crate) fn row_index_from_mouse(
    inner: ratatui::layout::Rect,
    column: u16,
    row: u16,
    row_count: usize,
    selected_row: usize,
) -> Option<usize> {
    if column < inner.x || column >= inner.x + inner.width {
        return None;
    }
    if row < inner.y || row >= inner.y + inner.height {
        return None;
    }
    // The list is rendered with a fresh ListState each frame, so ratatui scrolls
    // just enough to show the selected row: offset = selected + 1 - height.
    let offset = (selected_row + 1).saturating_sub(inner.height as usize);
    let idx = (row - inner.y) as usize + offset;
    (idx < row_count).then_some(idx)
}

pub(crate) fn draw_app_screen(
    f: &mut Frame,
    app_tab: AppTab,
    state: DrawState<'_>,
    cfg: &Config,
    settings: &SettingsForm,
    config_path: &str,
    discovered: &[cdamux_core::Project],
    projects_scroll: &mut usize,
    flat_sessions: &[EnrichedSession],
    sessions_selected: usize,
    sessions_scroll: &mut usize,
    agent_lines: &[String],
    waiting_only: bool,
    show_help: bool,
    fleet_tree_rect: &mut Option<ratatui::layout::Rect>,
    sidecar_preview_rect: &mut Option<ratatui::layout::Rect>,
) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(f.area());

    draw_tab_bar(f, root[0], app_tab);

    match app_tab {
        AppTab::Fleet if state.graph_view.is_some() => {
            draw_graph_screen(f, root[1], state.orch_graph, state.graph_view.unwrap_or(0));
        }
        AppTab::Fleet => draw_fleet_body(
            f,
            root[1],
            &state,
            cfg.sessions.group_by,
            waiting_only,
            fleet_tree_rect,
            sidecar_preview_rect,
        ),
        AppTab::Projects => draw_projects(
            f,
            root[1],
            cfg,
            discovered,
            projects_scroll,
            state.status_line,
        ),
        AppTab::Sessions => draw_sessions_list(
            f,
            root[1],
            flat_sessions,
            sessions_scroll,
            sessions_selected,
        ),
        AppTab::Agents => draw_agents(f, root[1], agent_lines),
        AppTab::Settings => draw_settings(f, root[1], cfg, settings, config_path),
    }

    let help = if app_tab == AppTab::Fleet && state.focus == FocusArea::Composer {
        "composer: type · Enter send+↵ · Esc back".to_string()
    } else if app_tab == AppTab::Fleet && state.graph_view.is_some() {
        GRAPH_FOOTER.to_string()
    } else {
        help_for_tab(app_tab, settings.dirty)
    };
    // Row 1: keys, trimmed to fit; row 2: status (never pushes the keys off screen).
    let help_line = fit_footer(&help, root[2].width as usize);
    f.render_widget(
        Paragraph::new(vec![
            Line::raw(help_line),
            Line::raw(state.status_line),
        ]),
        root[2],
    );

    if show_help {
        draw_help_overlay(f, f.area());
    }
}

pub(crate) fn draw_help_overlay(f: &mut Frame, area: ratatui::layout::Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title("keyboard help (? / Esc close)");
    let inner = block.inner(area);
    // The Fleet screen is drawn underneath: wipe it so it cannot show between columns.
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let text = fleet_help_text();
    if text.lines().count() <= inner.height as usize {
        f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        return;
    }
    // Short terminal: two columns, Navigate + Sessions | View + App.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);
    for (groups, col) in [(&HELP_LEFT, cols[0]), (&HELP_RIGHT, cols[1])] {
        f.render_widget(Paragraph::new(fleet_help_groups(groups)).wrap(Wrap { trim: false }), col);
    }
}

pub(crate) fn draw_fleet_body(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    state: &DrawState<'_>,
    group_by: GroupBy,
    waiting_only: bool,
    tree_rect: &mut Option<ratatui::layout::Rect>,
    preview_rect: &mut Option<ratatui::layout::Rect>,
) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4)])
        .split(area);

    let mut title_parts = vec![
        Span::styled("cdamux", Style::default().fg(CLR_ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" — fleet", Style::default().fg(CLR_TEXT)),
        Span::styled("  ○ idle ", Style::default().fg(CLR_IDLE)),
        Span::styled("● active ", Style::default().fg(CLR_OK)),
        Span::styled("◆ needs ", Style::default().fg(CLR_WARN)),
        Span::styled("⊘ blocked ", Style::default().fg(node_state_color(NodeState::Blocked))),
        Span::styled("✓ done", Style::default().fg(node_state_color(NodeState::Done))),
    ];
    if waiting_only {
        title_parts.push(Span::styled("  [waiting]", Style::default().fg(CLR_WARN)));
    }
    if group_by != GroupBy::Project {
        title_parts.push(Span::styled(
            format!("  group:{:?}", group_by),
            Style::default().fg(CLR_PROJECT),
        ));
    }
    let title = Paragraph::new(Line::from(title_parts)).block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title, inner[0]);

    let stacked = area.width < STACK_BELOW_COLS;
    let columns = if stacked {
        // Tree takes the rows it needs (title + rows + border), at most 45% of the body.
        let tree_h = (state.rows.len().max(1) as u16 + 2).min(inner[1].height * 45 / 100).max(3);
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(tree_h), Constraint::Min(0)])
            .split(inner[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(inner[1])
    };

    *tree_rect = Some(tree_list_block(stacked).inner(columns[0]));
    draw_tree_column(f, columns[0], state.rows, state.selected_row, stacked);
    draw_sidecar(f, columns[1], state, preview_rect);
}

pub(crate) fn draw_tree_column(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    rows: &[TreeRow],
    selected_row: usize,
    stacked: bool,
) {
    let items: Vec<ListItem> = if rows.is_empty() {
        vec![ListItem::new(Span::styled(
            "(no projects or sessions)",
            Style::default().fg(CLR_MUTED),
        ))]
    } else {
        rows.iter().map(tree_row_item).collect()
    };
    let len = items.len();
    let list = List::new(items)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(CLR_SEL_BG)
                .fg(CLR_TEXT)
                .add_modifier(Modifier::BOLD),
        )
        .block(tree_list_block(stacked));
    let mut list_state = ListState::default();
    list_state.select(Some(selected_row.min(rows.len().saturating_sub(1))));
    f.render_stateful_widget(list, area, &mut list_state);
    // RIGHT border column is reserved for the track (also when stacked + BOTTOM).
    draw_scrollbar(f, area, len, list_state.offset());
}

/// Depth 1 keeps the two-space indent; nested rows hang off their parent with `└ `.
fn tree_indent(depth: usize) -> String {
    if depth <= 1 {
        "  ".into()
    } else {
        format!("{}└ ", "  ".repeat(depth - 1))
    }
}

pub(crate) fn tree_row_item(row: &TreeRow) -> ListItem<'_> {
    match row {
        TreeRow::Project {
            name,
            path,
            session_count,
            worker_count,
            expanded,
            ..
        } => {
            let chevron = if *expanded { "▼" } else { "▶" };
            let counts = if *worker_count > 0 {
                format!("({session_count} sess · {worker_count} orch) ")
            } else {
                format!("({session_count}) ")
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", chevron), Style::default().fg(CLR_MUTED)),
                Span::styled("◇ ", Style::default().fg(CLR_ACCENT)),
                Span::styled(
                    format!("{} ", name),
                    Style::default()
                        .fg(CLR_PROJECT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(counts, Style::default().fg(CLR_MUTED)),
                Span::styled(path.clone(), Style::default().fg(CLR_MUTED)),
            ]))
        }
        TreeRow::Child { row, depth, .. } => {
            // Workers keep their gear while running so they read apart from sessions;
            // a blocked worker (state Error) uses the Graph / Steps blocked glyph and colour.
            let (icon, color) = match (row.kind, row.state) {
                (ChildKind::Worker, SessionState::Active) => ('⚙', CLR_OK),
                (ChildKind::Worker, SessionState::Idle) => ('·', CLR_IDLE),
                (ChildKind::Worker, SessionState::Error) => {
                    (NodeState::Blocked.icon(), node_state_color(NodeState::Blocked))
                }
                (ChildKind::Run, SessionState::Active) => ('◈', CLR_OK),
                (_, st) => (state_icon(st), session_state_color(st)),
            };
            // (bold, muted): workers lead with their issue id, sub-agents with their name.
            let (head, tail) = match row.kind {
                ChildKind::Worker => (format!("#{} ", row.id), row.label.clone()),
                ChildKind::Subagent => (
                    format!("{}  ", row.label),
                    row.id.chars().take(8).collect::<String>(),
                ),
                ChildKind::Run => (format!("run {} ", row.id), row.label.clone()),
            };
            ListItem::new(Line::from(vec![
                Span::raw(tree_indent(*depth)),
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(head, Style::default().fg(CLR_TEXT).add_modifier(Modifier::BOLD)),
                Span::styled(tail, Style::default().fg(CLR_MUTED)),
            ]))
        }
        TreeRow::Session {
            session: s,
            depth,
            expanded,
        } => {
            let icon = state_icon(s.state);
            let mut spans = vec![Span::raw(tree_indent(*depth))];
            if let Some(open) = expanded {
                let chevron = if *open { "▼ " } else { "▶ " };
                spans.push(Span::styled(chevron, Style::default().fg(CLR_MUTED)));
            }
            spans.extend([
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(session_state_color(s.state)),
                ),
                Span::styled(
                    format!("{}  ", s.name),
                    Style::default().fg(CLR_TEXT).add_modifier(Modifier::BOLD),
                ),
            ]);
            if let Some(n) = s.issue {
                spans.push(Span::styled(
                    format!("#{n}  "),
                    Style::default().fg(CLR_ACCENT),
                ));
            }
            spans.push(Span::styled(
                format!("{}  ", s.agent),
                Style::default().fg(CLR_MUTED),
            ));
            spans.push(Span::styled(s.path.clone(), Style::default().fg(CLR_MUTED)));
            ListItem::new(Line::from(spans))
        }
        TreeRow::UnassignedHeader => ListItem::new(Line::from(Span::styled(
            "— unassigned sessions",
            Style::default()
                .fg(CLR_WARN)
                .add_modifier(Modifier::ITALIC),
        ))),
    }
}

pub(crate) fn draw_sidecar(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    state: &DrawState,
    preview_rect: &mut Option<ratatui::layout::Rect>,
) {
    // Only a session row can receive text: projects / workers get the rows back.
    let show_composer = state.side_tab == SideTab::Preview && state.composer_session.is_some();
    // The chrome box needs tabs + chrome + a 4-row body + composer; drop it on short panes.
    let show_chrome = state.side_tab == SideTab::Preview
        && state.session_chrome.is_some()
        && area.height >= SIDECAR_CHROME_MIN_H;
    let vertical = if show_composer {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(if show_chrome { CHROME_H } else { 0 }),
                Constraint::Min(1),
                // Top border (with the label) + one input row.
                Constraint::Length(2),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(if show_chrome { CHROME_H } else { 0 }),
                Constraint::Min(1),
            ])
            .split(area)
    };

    let tabs = Line::from(
        [
            SideTab::Preview,
            SideTab::Notes,
            SideTab::Review,
            SideTab::OrchGraph,
        ]
            .iter()
            .map(|t| {
                let style = if *t == state.side_tab {
                    Style::default()
                        .fg(CLR_ACCENT)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(CLR_MUTED)
                };
                Span::styled(format!(" {} ", t.label()), style)
            })
            .collect::<Vec<_>>(),
    );
    f.render_widget(Paragraph::new(tabs), vertical[0]);

    // Layout is always [tabs, chrome?, body, composer?]. Chrome may be height 0
    // when hidden — body is still index 2 (was wrongly 1 → zero-height / black panel).
    let preview_idx = 2;
    if show_chrome {
        let chrome = state.session_chrome.unwrap();
        let chrome_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(CLR_MUTED))
            .title(Span::styled(
                " session ",
                Style::default().fg(CLR_ACCENT).add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(CLR_SEL_BG));
        let inner = chrome_block.inner(vertical[1]);
        f.render_widget(chrome_block, vertical[1]);
        f.render_widget(
            Paragraph::new(chrome.lines().join("\n")).style(
                Style::default()
                    .fg(CLR_TEXT)
                    .bg(CLR_SEL_BG)
                    .add_modifier(Modifier::DIM),
            ),
            inner,
        );
    }

    *preview_rect = Some(vertical[preview_idx]);
    let view_rows = vertical[preview_idx].height;
    if state.side_tab == SideTab::OrchGraph {
        draw_orch_graph_panel(f, vertical[preview_idx], state.orch_graph, None);
        return;
    }
    let body = if state.side_tab == SideTab::Preview {
        let para = Paragraph::new(preview_display_lines(
            state.preview_text,
            state.preview_scroll,
            view_rows,
        ))
            .wrap(Wrap { trim: false });
        // Wrapped lines take more rows than the window holds: keep the newest at the bottom.
        let overflow = para.line_count(vertical[preview_idx].width).saturating_sub(view_rows as usize);
        para.scroll((overflow as u16, 0))
    } else {
        Paragraph::new(sidecar_plain_body(
            state.side_tab,
            state.notes_text,
            state.review_text,
            &state.orch_graph.text,
        ))
            .style(Style::default().fg(CLR_TEXT))
            .wrap(Wrap { trim: false })
    };
    f.render_widget(body, vertical[preview_idx]);

    if show_composer {
        let composer_idx = preview_idx + 1;
        let area = vertical[composer_idx];
        let target = state
            .composer_session
            .map(|n| format!("→ {}", n))
            .unwrap_or_else(|| "→ (no session)".to_string());
        let border_color = if state.focus == FocusArea::Composer {
            CLR_ACCENT
        } else {
            CLR_MUTED
        };
        let editing = state.focus == FocusArea::Composer;
        let avail = (area.width as usize).saturating_sub(target.chars().count() + 3);
        let line = Line::from(vec![
            Span::raw(format!("{target} ")),
            text_field(state.composer_line, editing, avail),
        ]);
        let composer = Paragraph::new(line).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(border_color))
                .title(COMPOSER_LABEL),
        );
        f.render_widget(composer, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdamux_core::EnrichedSession;

    fn session_rows(n: usize) -> Vec<TreeRow> {
        (0..n)
            .map(|i| TreeRow::Session {
                session: EnrichedSession {
                    name: format!("s{i:02}"),
                    ..Default::default()
                },
                depth: 1,
                expanded: None,
            })
            .collect()
    }

    /// Rightmost column of the tree area (RIGHT border / scrollbar track).
    fn tree_right_column(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        let buf = term.backend().buffer();
        let x = buf.area.width - 1;
        (0..buf.area.height).map(|y| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn tree_scrollbar_only_when_overflowing() {
        for stacked in [false, true] {
            let mut term =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 12)).unwrap();
            term.draw(|f| draw_tree_column(f, f.area(), &session_rows(5), 0, stacked))
                .unwrap();
            let col = tree_right_column(&term);
            assert!(
                col.chars().all(|c| c == '│' || c == '┘' || c == '─'),
                "stacked={stacked}: no scrollbar expected: {col}"
            );
            // Last content column must stay clear of the thumb (RIGHT border reserved).
            let buf = term.backend().buffer();
            let x = buf.area.width - 2;
            let content_col: String = (0..buf.area.height).map(|y| buf[(x, y)].symbol()).collect();
            assert!(
                !content_col.contains('█'),
                "stacked={stacked}: thumb must not overwrite text: {content_col}"
            );

            term.draw(|f| draw_tree_column(f, f.area(), &session_rows(30), 29, stacked))
                .unwrap();
            let col = tree_right_column(&term);
            assert!(
                col.contains('█'),
                "stacked={stacked}: scrollbar thumb expected: {col}"
            );
            let buf = term.backend().buffer();
            let x = buf.area.width - 2;
            let content_col: String = (0..buf.area.height).map(|y| buf[(x, y)].symbol()).collect();
            assert!(
                !content_col.contains('█'),
                "stacked={stacked}: thumb must stay on border column: {content_col}"
            );
        }
    }

    #[test]
    fn mouse_click_accounts_for_scroll_offset() {
        let area = ratatui::layout::Rect::new(0, 0, 40, 20);
        let inner = tree_list_block(false).inner(area);
        let h = inner.height as usize;
        // Not scrolled: first visible row is row 0.
        assert_eq!(row_index_from_mouse(inner, inner.x, inner.y, 30, 0), Some(0));
        // Row `h + 4` selected: offset is 5, so the top visible row is row 5.
        assert_eq!(row_index_from_mouse(inner, inner.x, inner.y, 30, h + 4), Some(5));
        // Click past the last row, or on the border column.
        assert_eq!(row_index_from_mouse(inner, inner.x, inner.y + 2, 2, 0), None);
        assert_eq!(row_index_from_mouse(inner, area.width - 1, inner.y, 30, 0), None);
    }

    #[test]
    fn preview_scroll_is_clamped_both_ways() {
        // 10 lines, 4 visible: from the tail (0) up to the first line (6).
        assert_eq!(scroll_preview(0, 3, 10, 4), 3);
        assert_eq!(scroll_preview(5, 3, 10, 4), 6);
        assert_eq!(scroll_preview(2, -3, 10, 4), 0);
        // Capture shorter than the view: never scrolls.
        assert_eq!(scroll_preview(0, 3, 2, 4), 0);
    }

    #[test]
    fn side_tab_cycles() {
        assert_eq!(SideTab::Preview.next(), SideTab::Notes);
        assert_eq!(SideTab::Review.next(), SideTab::OrchGraph);
        assert_eq!(SideTab::OrchGraph.next(), SideTab::Preview);
    }

    #[test]
    fn preview_renders_ansi_color() {
        let raw = "\x1b[32mgreen\x1b[0m";
        let lines = preview_display_lines(raw, 0, 10);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn preview_window_anchors_to_tail() {
        let raw = (0..10)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = preview_display_lines(&raw, 0, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines[2].to_string().contains("line9"));
        assert!(lines[0].to_string().contains("line7"));
    }
}
