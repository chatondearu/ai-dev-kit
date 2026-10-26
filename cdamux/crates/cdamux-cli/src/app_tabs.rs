//! Application-level tabs (Fleet, Projects, Sessions, Agents, Settings).

use cdamux_core::{
    AttachMode, Config, EnrichedSession, GroupBy, Project, enrich_sessions, list_tmux_sessions,
    save_config,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
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
const CLR_SEL_BG: Color = Color::Rgb(42, 49, 64);
const CLR_PANEL: Color = Color::Rgb(20, 24, 32);

fn screen_block(title: &str) -> Block<'static> {
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

pub fn help_for_tab(tab: AppTab, settings_dirty: bool) -> &'static str {
    match tab {
        AppTab::Fleet => {
            "↑/k ↓/j · [/] preview scroll · Space composer · Enter attach · w/F/g · ? · 1-5 · q"
        }
        AppTab::Projects => "↑/k ↓/j scroll · 1-5 tabs · q quit",
        AppTab::Sessions => "↑/k ↓/j · Enter focus in Fleet · 1-5 tabs · q quit",
        AppTab::Agents => "1-5 tabs · q quit",
        AppTab::Settings => {
            if settings_dirty {
                "a attach · g group_by · +/- preview · t tailnet · S save · 1-5 tabs · q quit  (unsaved)"
            } else {
                "a attach · g group_by · +/- preview · t tailnet · S save · 1-5 tabs · q quit"
            }
        }
    }
}

pub fn draw_projects(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    cfg: &Config,
    discovered: &[Project],
    scroll: usize,
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
        "roots: (none — add under [projects.roots] in config)".to_string()
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

    let visible = discovered.len().min(chunks[1].height as usize);
    let start = scroll.min(discovered.len().saturating_sub(visible.max(1)));
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
}

pub fn draw_sessions_list(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    sessions: &[EnrichedSession],
    scroll: usize,
    selected: usize,
) {
    let visible = area.height.saturating_sub(2) as usize;
    let start = scroll.min(sessions.len().saturating_sub(visible.max(1)));
    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .skip(start)
        .take(visible.max(1))
        .map(|(i, s)| {
            let global = start + i;
            let sel = global == selected;
            let style = if sel {
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
    f.render_widget(
        List::new(if items.is_empty() {
            vec![ListItem::new("(no tmux sessions)")]
        } else {
            items
        }).block(screen_block(&format!("tmux sessions ({})", sessions.len()))),
        area,
    );
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

pub fn draw_settings(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    cfg: &Config,
    dirty: bool,
    status: &str,
    config_path: &str,
) {
    let attach = match cfg.sessions.attach_mode {
        AttachMode::Mirror => "mirror",
        AttachMode::Exclusive => "exclusive",
    };
    let tailnet = if cfg.daemon.listen_tailnet {
        format!("on (port {})", cfg.daemon.tailnet_port)
    } else {
        "off".into()
    };
    let dirty_tag = if dirty { " *" } else { "" };
    let status_msg = if status.is_empty() {
        "S: save to disk"
    } else {
        status
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("config: ", Style::default().fg(CLR_MUTED)),
            Span::styled(
                format!("{config_path}{dirty_tag}"),
                Style::default().fg(CLR_TEXT),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("[sessions]", Style::default().fg(CLR_ACCENT))),
        Line::from(vec![
            Span::raw("  attach_mode = "),
            Span::styled(attach, Style::default().fg(CLR_PROJECT)),
            Span::styled("  (a)", Style::default().fg(CLR_MUTED)),
        ]),
        Line::from(vec![
            Span::raw("  group_by = "),
            Span::styled(format!("{:?}", cfg.sessions.group_by), Style::default().fg(CLR_PROJECT)),
            Span::styled("  (g)", Style::default().fg(CLR_MUTED)),
        ]),
        Line::from(""),
        Line::from(Span::styled("[ui]", Style::default().fg(CLR_ACCENT))),
        Line::from(vec![
            Span::raw("  preview_max_lines = "),
            Span::styled(
                cfg.ui.preview_max_lines.to_string(),
                Style::default().fg(CLR_PROJECT),
            ),
            Span::styled("  (+/-)", Style::default().fg(CLR_MUTED)),
        ]),
        Line::from(""),
        Line::from(Span::styled("[daemon]", Style::default().fg(CLR_ACCENT))),
        Line::from(vec![
            Span::raw("  listen_tailnet = "),
            Span::styled(
                tailnet,
                Style::default().fg(if cfg.daemon.listen_tailnet {
                    CLR_OK
                } else {
                    CLR_MUTED
                }),
            ),
            Span::styled("  (t)", Style::default().fg(CLR_MUTED)),
        ]),
        Line::from(vec![
            Span::raw("  idle_seconds_for_needs_input = "),
            Span::styled(
                cfg.daemon.idle_seconds_for_needs_input.to_string(),
                Style::default().fg(CLR_TEXT),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(status_msg, Style::default().fg(CLR_WARN))),
    ];
    f.render_widget(
        Paragraph::new(lines).block(screen_block("settings")),
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

pub fn cycle_attach_mode(mode: AttachMode) -> AttachMode {
    match mode {
        AttachMode::Mirror => AttachMode::Exclusive,
        AttachMode::Exclusive => AttachMode::Mirror,
    }
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

pub fn bump_preview_lines(lines: u32, delta: i32) -> u32 {
    let presets = [40u32, 80, 120, 160, 240];
    let cur = lines.clamp(10, 500);
    let pos = presets
        .iter()
        .position(|&p| p == cur)
        .unwrap_or_else(|| presets.iter().position(|&p| p >= cur).unwrap_or(1));
    let next = (pos as i32 + delta).clamp(0, presets.len() as i32 - 1);
    presets[next as usize]
}

pub fn try_save(cfg: &Config) -> Result<String, String> {
    save_config(cfg).map(|_| "saved config".to_string()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_digits() {
        assert_eq!(AppTab::from_digit('3'), Some(AppTab::Sessions));
        assert_eq!(AppTab::from_digit('9'), None);
    }

    #[test]
    fn group_by_cycles() {
        assert_eq!(cycle_group_by(GroupBy::Project), GroupBy::Cwd);
        assert_eq!(cycle_group_by(GroupBy::None), GroupBy::Project);
    }
}
