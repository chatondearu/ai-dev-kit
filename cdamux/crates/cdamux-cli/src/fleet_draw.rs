//! Drawing code for the fleet dashboard: tabs, tree column, sidecar and overlays.

use crate::app_tabs::{
    draw_agents, draw_projects, draw_sessions_list, draw_tab_bar, help_for_tab, AppTab,
};
use crate::fleet_tree::TreeRow;
use crate::keymap::fleet_help_text;
use crate::settings_form::{draw_settings, SettingsForm};
use crate::spawn_modal::centered_rect;
use ansi_to_tui::IntoText as _;
use cdamux_core::{
    state_icon, worker_phase_icon, Config, EnrichedSession, GroupBy, SessionChrome, SessionState,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap},
    Frame,
};
use std::path::Path;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusArea {
    Tree,
    Composer,
    PreviewScroll,
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

pub(crate) fn draw_rename_modal(f: &mut Frame, state: &RenameState) {
    const MODAL_BG: Color = Color::Rgb(22, 26, 34);
    const MODAL_FG: Color = Color::Rgb(230, 234, 240);
    let area = centered_rect(60, 25, f.area());
    let block = Block::default()
        .title(" Rename session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(MODAL_BG).fg(MODAL_FG));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Block::default().style(Style::default().bg(MODAL_BG)), inner);
    let body = format!(
        "from: {}\nnew name: {}█\nEnter save · Esc cancel",
        state.old_name,
        state.input
    );
    f.render_widget(
        Paragraph::new(body).style(Style::default().bg(MODAL_BG).fg(MODAL_FG)),
        inner,
    );
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

pub(crate) fn sidecar_plain_body(tab: SideTab, notes: &str, review: &str, orch_graph: &str) -> String {
    match tab {
        SideTab::Preview => String::new(),
        SideTab::Notes => notes.to_string(),
        SideTab::Review => review.to_string(),
        SideTab::OrchGraph => orch_graph.to_string(),
    }
}

pub(crate) fn orch_graph_text_for_cwd(cwd: &str, width: usize) -> String {
    if cwd.is_empty() {
        return "Select a project or session with a live orch run.".to_string();
    }
    match cdamux_core::find_orch_graph_from_cwd(cwd) {
        Ok((g, age)) => {
            cdamux_core::format_orch_graph_panel_with_history(&g, width, age, Path::new(cwd))
        }
        Err(cdamux_core::OrchGraphError::NotFound(_)) => {
            let runs = cdamux_core::list_orch_graph_runs(Path::new(cwd), 5);
            if runs.is_empty() {
                "(no live graph — start orch.sh in this repo; tab cycles with sidecar)".to_string()
            } else {
                let mut s = String::from("(no live graph)\n\nrecent runs:\n");
                for r in runs {
                    let st = r.status.unwrap_or_else(|| "?".into());
                    s.push_str(&format!("  {} · {}\n", r.run_id, st));
                }
                s
            }
        }
        Err(cdamux_core::OrchGraphError::Invalid { path, message }) => {
            format!("invalid current.json at {}:\n{}", path.display(), message)
        }
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
    pub(crate) orch_graph_text: &'a str,
    pub(crate) composer_line: &'a str,
    pub(crate) composer_session: Option<&'a str>,
    pub(crate) session_chrome: Option<&'a SessionChrome>,
    pub(crate) status_line: &'a str,
}

pub(crate) fn session_state_color(state: SessionState) -> Color {
    match state {
        SessionState::Idle => CLR_IDLE,
        SessionState::Active => CLR_OK,
        SessionState::NeedsInput => CLR_WARN,
        SessionState::Done => CLR_ACCENT,
        SessionState::Error => CLR_WARN,
    }
}

fn tree_list_block() -> Block<'static> {
    Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(CLR_MUTED))
        .title(Span::styled(
            " projects → sessions ",
            Style::default().fg(CLR_ACCENT).add_modifier(Modifier::BOLD),
        ))
}

pub(crate) fn row_index_from_mouse(
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
    row_count: usize,
) -> Option<usize> {
    let inner = tree_list_block().inner(area);
    if column < inner.x || column >= inner.x + inner.width {
        return None;
    }
    if row < inner.y || row >= inner.y + inner.height {
        return None;
    }
    let rel = (row - inner.y) as usize;
    if rel < row_count {
        Some(rel)
    } else {
        None
    }
}

pub(crate) fn draw_app_screen(
    f: &mut Frame,
    app_tab: AppTab,
    state: DrawState<'_>,
    cfg: &Config,
    settings: &SettingsForm,
    config_path: &str,
    discovered: &[cdamux_core::Project],
    projects_scroll: usize,
    flat_sessions: &[EnrichedSession],
    sessions_selected: usize,
    sessions_scroll: usize,
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
    } else {
        help_for_tab(app_tab, settings.dirty)
    };
    let help_line = if state.status_line.is_empty() {
        help
    } else {
        format!("{}  |  {}", help, state.status_line)
    };
    f.render_widget(Paragraph::new(help_line), root[2]);

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
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(fleet_help_text()).wrap(Wrap { trim: false }),
        inner,
    );
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
        Span::styled("◆ needs", Style::default().fg(CLR_WARN)),
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

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(inner[1]);

    *tree_rect = Some(columns[0]);
    draw_tree_column(f, columns[0], state.rows, state.selected_row);
    draw_sidecar(f, columns[1], state, preview_rect);
}

pub(crate) fn draw_tree_column(f: &mut Frame, area: ratatui::layout::Rect, rows: &[TreeRow], selected_row: usize) {
    let items: Vec<ListItem> = if rows.is_empty() {
        vec![ListItem::new(Span::styled(
            "(no projects or sessions)",
            Style::default().fg(CLR_MUTED),
        ))]
    } else {
        rows.iter().map(tree_row_item).collect()
    };
    let list = List::new(items)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(CLR_SEL_BG)
                .fg(CLR_TEXT)
                .add_modifier(Modifier::BOLD),
        )
        .block(tree_list_block());
    let mut list_state = ListState::default();
    list_state.select(Some(selected_row.min(rows.len().saturating_sub(1))));
    StatefulWidget::render(list, area, f.buffer_mut(), &mut list_state);
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
        TreeRow::Worker {
            id,
            phase,
            message,
            ..
        } => {
            let icon = worker_phase_icon(phase);
            let color = if phase == "blocked" {
                CLR_WARN
            } else if phase == "done" {
                CLR_ACCENT
            } else {
                CLR_MUTED
            };
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(
                    format!("#{id} "),
                    Style::default().fg(CLR_TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{phase}  "), Style::default().fg(CLR_MUTED)),
                Span::styled(message.clone(), Style::default().fg(CLR_MUTED)),
            ]))
        }
        TreeRow::Session(s) | TreeRow::Unassigned(s) => {
            let icon = state_icon(s.state);
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(session_state_color(s.state)),
                ),
                Span::styled(
                    format!("{}  ", s.name),
                    Style::default().fg(CLR_TEXT).add_modifier(Modifier::BOLD),
                ),
            ];
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
    let show_composer = state.side_tab == SideTab::Preview;
    let show_chrome = state.side_tab == SideTab::Preview && state.session_chrome.is_some();
    let vertical = if show_composer {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(if show_chrome { 3 } else { 0 }),
                Constraint::Min(4),
                Constraint::Length(3),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(if show_chrome { 3 } else { 0 }),
                Constraint::Min(4),
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
    let body_block = Block::default().borders(Borders::NONE).title(" ");
    let view_rows = vertical[preview_idx].height;
    let body = if state.side_tab == SideTab::Preview {
        Paragraph::new(preview_display_lines(
            state.preview_text,
            state.preview_scroll,
            view_rows,
        ))
            .wrap(Wrap { trim: false })
            .block(body_block)
    } else {
        Paragraph::new(sidecar_plain_body(
            state.side_tab,
            state.notes_text,
            state.review_text,
            state.orch_graph_text,
        ))
            .style(Style::default().fg(CLR_TEXT))
            .wrap(Wrap { trim: false })
            .block(body_block)
    };
    f.render_widget(body, vertical[preview_idx]);

    if show_composer {
        let composer_idx = preview_idx + 1;
        let target = state
            .composer_session
            .map(|n| format!("→ {}", n))
            .unwrap_or_else(|| "→ (no session)".to_string());
        let border_color = if state.focus == FocusArea::Composer {
            CLR_ACCENT
        } else {
            CLR_MUTED
        };
        let line = if state.composer_line.is_empty() {
            format!("{} _", target)
        } else {
            format!("{} {}", target, state.composer_line)
        };
        let composer = Paragraph::new(line).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(border_color))
                .title(COMPOSER_LABEL),
        );
        f.render_widget(composer, vertical[composer_idx]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
