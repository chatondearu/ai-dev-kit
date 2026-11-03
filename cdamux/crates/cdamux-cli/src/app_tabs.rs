//! Application-level tabs (Fleet, Projects, Sessions, Agents, Settings).

use crate::keymap::fleet_footer;
use crate::settings_form::SETTINGS_FOOTER;
use crate::widgets::{draw_scrolled_list, draw_scrollbar};
use cdamux_core::{
    Config, EnrichedSession, GroupBy, Project, enrich_sessions, list_tmux_sessions,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub const CLR_TEXT: Color = Color::Rgb(230, 234, 240);
pub const CLR_MUTED: Color = Color::Rgb(139, 149, 168);
pub const CLR_ACCENT: Color = Color::Rgb(110, 181, 255);
pub const CLR_PROJECT: Color = Color::Rgb(158, 200, 255);
pub const CLR_OK: Color = Color::Rgb(107, 207, 142);
pub const CLR_WARN: Color = Color::Rgb(232, 184, 74);
pub const CLR_SEL_BG: Color = Color::Rgb(42, 49, 64);
const CLR_PANEL: Color = Color::Rgb(20, 24, 32);

pub fn screen_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CLR_MUTED))
        .style(Style::default().bg(CLR_PANEL))
        .title(
            Line::from(Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(CLR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            )),
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Fleet,
    Projects,
    Sessions,
    Agents,
    Settings,
}

impl AppTab {
    pub const ALL: [AppTab; 5] = [
        AppTab::Fleet,
        AppTab::Projects,
        AppTab::Sessions,
        AppTab::Agents,
        AppTab::Settings,
    ];

    pub fn from_digit(c: char) -> Option<Self> {
        match c {
            '1' => Some(AppTab::Fleet),
            '2' => Some(AppTab::Projects),
            '3' => Some(AppTab::Sessions),
            '4' => Some(AppTab::Agents),
            '5' => Some(AppTab::Settings),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            AppTab::Fleet => "Fleet",
            AppTab::Projects => "Projects",
            AppTab::Sessions => "Sessions",
            AppTab::Agents => "Agents",
            AppTab::Settings => "Settings",
        }
    }

    fn digit(self) -> char {
        match self {
            AppTab::Fleet => '1',
            AppTab::Projects => '2',
            AppTab::Sessions => '3',
            AppTab::Agents => '4',
            AppTab::Settings => '5',
        }
    }
}

pub fn draw_tab_bar(f: &mut Frame, area: ratatui::layout::Rect, active: AppTab) {
    let spans: Vec<Span> = AppTab::ALL
        .iter()
        .flat_map(|tab| {
            let style = if *tab == active {
                Style::default()
                    .fg(CLR_ACCENT)
                    .bg(CLR_SEL_BG)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(CLR_MUTED)
            };
            [
                Span::styled(format!(" {}:{} ", tab.digit(), tab.label()), style),
                Span::raw(" "),
            ]
        })
        .collect();
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub fn help_for_tab(tab: AppTab, settings_dirty: bool) -> String {
    match tab {
        AppTab::Fleet => fleet_footer(),
        AppTab::Projects => "↑/k ↓/j scroll · 1-5 tabs · q quit".into(),
        AppTab::Sessions => "↑/k ↓/j · Enter focus in Fleet · 1-5 tabs · q quit".into(),
        AppTab::Agents => "1-5 tabs · q quit".into(),
        AppTab::Settings if settings_dirty => format!("{SETTINGS_FOOTER}  (unsaved)"),
        AppTab::Settings => SETTINGS_FOOTER.into(),
    }
}

pub fn draw_projects(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    cfg: &Config,
    discovered: &[Project],
    scroll: &mut usize,
    status: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(4)])
        .split(area);

    let roots: Vec<String> = cfg
        .projects
        .roots
        .iter()
        .map(|r| {
            let label = if r.label.is_empty() {
                r.path.display().to_string()
            } else {
                format!("{} → {}", r.label, r.path.display())
            };
            label
        })
        .collect();
    let roots_block = if roots.is_empty() {
        "roots: (none — add one in Settings (5) with `a`)".to_string()
    } else {
        format!(
            "discover={:?} max_depth={}\nroots:\n{}",
            cfg.projects.discover,
            cfg.projects.max_depth,
            roots.join("\n")
        )
    };
    f.render_widget(
        Paragraph::new(roots_block)
            .style(Style::default().fg(CLR_TEXT))
            .block(screen_block("roots")),
        chunks[0],
    );

    // Minus the 2 border rows, or the last projects stay unreachable.
    let visible = discovered.len().min(chunks[1].height.saturating_sub(2) as usize);
    let start = (*scroll).min(discovered.len().saturating_sub(visible.max(1)));
    // Write the clamp back so `k` reacts at once after overshooting with `j`.
    *scroll = start;
    let items: Vec<ListItem> = discovered
        .iter()
        .skip(start)
        .take(visible.max(1))
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{}  ", p.name),
                    Style::default().fg(CLR_PROJECT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(p.path.display().to_string(), Style::default().fg(CLR_MUTED)),
            ]))
        })
        .collect();
    let title = if status.is_empty() {
        format!("discovered ({})", discovered.len())
    } else {
        format!("discovered ({}) — {}", discovered.len(), status)
    };
    f.render_widget(
        List::new(if items.is_empty() {
            vec![ListItem::new("(no repos under roots)")]
        } else {
            items
        }).block(screen_block(&title)),
        chunks[1],
    );
    draw_scrollbar(f, chunks[1].inner(Margin { vertical: 1, horizontal: 0 }), discovered.len(), start);
}

pub fn draw_sessions_list(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    sessions: &[EnrichedSession],
    scroll: &mut usize,
    selected: usize,
) {
    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == selected {
                Style::default()
                    .bg(CLR_SEL_BG)
                    .fg(CLR_TEXT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(CLR_TEXT)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}  ", s.name), style),
                Span::styled(s.path.clone(), Style::default().fg(CLR_MUTED)),
            ]))
        })
        .collect();
    let block = screen_block(&format!("tmux sessions ({})", sessions.len()));
    if items.is_empty() {
        f.render_widget(List::new(vec![ListItem::new("(no tmux sessions)")]).block(block), area);
    } else {
        draw_scrolled_list(f, area, List::new(items).block(block), selected, scroll, true);
    }
}

pub fn draw_agents(f: &mut Frame, area: ratatui::layout::Rect, lines: &[String]) {
    if lines.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("(no agent info)", Style::default().fg(CLR_MUTED)))
                .block(screen_block("agents & PATH")),
            area,
        );
        return;
    }
    let colored = lines
        .iter()
        .map(|line| {
            let style = if line.contains("MISSING") {
                Style::default().fg(CLR_WARN)
            } else if line.contains(": ok") {
                Style::default().fg(CLR_OK)
            } else if line.starts_with("agent ") {
                Style::default().fg(CLR_TEXT)
            } else {
                Style::default().fg(CLR_ACCENT)
            };
            Line::from(Span::styled(line.as_str(), style))
        })
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(colored)
            .wrap(Wrap { trim: false })
            .block(screen_block("agents & PATH")),
        area,
    );
}

pub fn flat_sessions(cfg: &Config) -> Vec<EnrichedSession> {
    let raw = list_tmux_sessions().unwrap_or_default();
    let mut sessions = enrich_sessions(
        &raw,
        cfg.sessions.group_by,
        &cfg.projects,
        cfg.daemon.idle_seconds_for_needs_input,
    );
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    sessions
}

pub fn cycle_group_by(g: GroupBy) -> GroupBy {
    match g {
        GroupBy::Project => GroupBy::Cwd,
        GroupBy::Cwd => GroupBy::Agent,
        GroupBy::Agent => GroupBy::GroupPath,
        GroupBy::GroupPath => GroupBy::None,
        GroupBy::None => GroupBy::Project,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_digits() {
        assert_eq!(AppTab::from_digit('3'), Some(AppTab::Sessions));
        assert_eq!(AppTab::from_digit('9'), None);
    }

    use crate::test_render::buffer_lines;

    fn sessions(n: usize) -> Vec<EnrichedSession> {
        (0..n)
            .map(|i| EnrichedSession {
                name: format!("s{i:02}"),
                path: String::new(),
                group_key: String::new(),
                state: cdamux_core::SessionState::Idle,
                agent: String::new(),
                issue: None,
                ..Default::default()
            })
            .collect()
    }

    /// Right column, inside the top/bottom borders.
    fn right_column(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        let buf = term.backend().buffer();
        let x = buf.area.width - 1;
        (1..buf.area.height - 1).map(|y| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn sessions_scrollbar_only_when_overflowing() {
        let mut term =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 12)).unwrap();
        let mut scroll = 0;
        term.draw(|f| draw_sessions_list(f, f.area(), &sessions(5), &mut scroll, 0)).unwrap();
        assert!(right_column(&term).chars().all(|c| c == '│'), "no scrollbar expected");
        term.draw(|f| draw_sessions_list(f, f.area(), &sessions(30), &mut scroll, 29)).unwrap();
        // Last offset: thumb sits at the bottom of the track.
        assert!(right_column(&term).ends_with('█'), "{}", right_column(&term));
    }

    #[test]
    fn sessions_list_follows_cursor_both_ways() {
        let sessions = sessions(30);
        // 12 rows = 10 list rows + 2 borders.
        let mut term =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 12)).unwrap();
        let mut scroll = 0;
        let mut draw = |selected: usize, scroll: &mut usize| {
            term.draw(|f| draw_sessions_list(f, f.area(), &sessions, scroll, selected)).unwrap();
            let lines = buffer_lines(&term);
            let row = lines.iter().position(|l| l.contains(&format!("s{selected:02}"))).unwrap();
            let cell = &term.backend().buffer()[(1, row as u16)];
            assert_eq!(cell.bg, CLR_SEL_BG, "s{selected:02} not highlighted");
            assert!(right_column(&term).contains('█'), "scrollbar thumb expected");
            lines
        };
        let lines = draw(25, &mut scroll);
        assert!(!lines.iter().any(|l| l.contains("s00")));
        assert_eq!(scroll, 16);
        // Moving up inside the viewport keeps the offset (no pinning to the top row).
        draw(20, &mut scroll);
        assert_eq!(scroll, 16);
        // Past the top edge the list scrolls back up.
        draw(3, &mut scroll);
        assert_eq!(scroll, 3);
    }

    #[test]
    fn projects_last_row_reachable() {
        let projects: Vec<Project> = (0..30)
            .map(|i| Project {
                name: format!("p{i:02}"),
                path: format!("/src/p{i:02}").into(),
                root_label: String::new(),
            })
            .collect();
        let mut term =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 18)).unwrap();
        let mut scroll = usize::MAX;
        term.draw(|f| draw_projects(f, f.area(), &Config::default(), &projects, &mut scroll, ""))
            .unwrap();
        assert!(buffer_lines(&term).iter().any(|l| l.contains("p29")));
        assert!(right_column(&term).ends_with('█'));
        // The overshoot is clamped so `k` scrolls back at once.
        assert_eq!(scroll, 30 - (18 - 4 - 2));
    }

    #[test]
    fn group_by_cycles() {
        assert_eq!(cycle_group_by(GroupBy::Project), GroupBy::Cwd);
        assert_eq!(cycle_group_by(GroupBy::None), GroupBy::Project);
    }
}
