//! Full-screen ratatui dashboard (default `cdamux` with no subcommand).

use cdamux_core::{
    grouped_for_list, load_config, EnrichedSession, GroupBy, GroupedSessions, SessionState,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, stdout, Stdout};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    None,
    Quit,
    Attach,
    Spawn,
    CycleGroupBy,
    SelectNext,
    SelectPrev,
}

pub fn map_key_event(key: KeyEvent) -> DashboardAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return DashboardAction::Quit;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => DashboardAction::Quit,
        KeyCode::Char('g') => DashboardAction::CycleGroupBy,
        KeyCode::Char('n') | KeyCode::Char('s') => DashboardAction::Spawn,
        KeyCode::Enter => DashboardAction::Attach,
        KeyCode::Down | KeyCode::Char('j') => DashboardAction::SelectNext,
        KeyCode::Up | KeyCode::Char('k') => DashboardAction::SelectPrev,
        _ => DashboardAction::None,
    }
}

pub fn next_group_by(current: GroupBy) -> GroupBy {
    match current {
        GroupBy::Project => GroupBy::Cwd,
        GroupBy::Cwd => GroupBy::Agent,
        GroupBy::Agent => GroupBy::GroupPath,
        GroupBy::GroupPath => GroupBy::None,
        GroupBy::None => GroupBy::Project,
    }
}

struct FlatRow {
    is_header: bool,
    label: String,
    session: Option<EnrichedSession>,
}

fn flatten_grouped(grouped: &GroupedSessions) -> Vec<FlatRow> {
    let show_headers = grouped.group_by != GroupBy::None;
    let mut rows = Vec::new();
    for group in &grouped.groups {
        if show_headers {
            rows.push(FlatRow {
                is_header: true,
                label: group.key.clone(),
                session: None,
            });
        }
        for s in &group.sessions {
            rows.push(FlatRow {
                is_header: false,
                label: s.name.clone(),
                session: Some(s.clone()),
            });
        }
    }
    rows
}

fn session_indices(rows: &[FlatRow]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| !r.is_header && r.session.is_some())
        .map(|(i, _)| i)
        .collect()
}

fn clamp_selection(rows: &[FlatRow], selected_row: usize) -> usize {
    let indices = session_indices(rows);
    if indices.is_empty() {
        return 0;
    }
    if indices.contains(&selected_row) {
        return selected_row;
    }
    indices[0]
}

fn move_selection(rows: &[FlatRow], selected_row: usize, delta: i32) -> usize {
    let indices = session_indices(rows);
    if indices.is_empty() {
        return 0;
    }
    let pos = indices
        .iter()
        .position(|&i| i == selected_row)
        .unwrap_or(0);
    let next = (pos as i32 + delta).clamp(0, indices.len() as i32 - 1);
    indices[next as usize]
}

fn state_label(state: SessionState) -> &'static str {
    match state {
        SessionState::Idle => "idle",
        SessionState::Active => "active",
        SessionState::NeedsInput => "needs_input",
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<(Terminal<CrosstermBackend<Stdout>>, Self)> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(stdout(), crossterm::terminal::EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        Ok((terminal, Self))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub fn run_dashboard() -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let mut group_by = cfg.sessions.group_by;
    let mut selected_row = 0;

    let (mut terminal, _guard) = match TerminalGuard::enter() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("dashboard: terminal setup failed: {e}");
            return ExitCode::from(1);
        }
    };

    let mut should_quit = false;
    let mut attach_name: Option<String> = None;
    let mut spawn_requested = false;

    while !should_quit && attach_name.is_none() && !spawn_requested {
        let grouped = grouped_for_list(
            group_by,
            &cfg.projects,
            cfg.daemon.idle_seconds_for_needs_input,
        );
        let rows = flatten_grouped(&grouped);
        selected_row = clamp_selection(&rows, selected_row);

        terminal
            .draw(|f| draw_dashboard(f, &grouped, &rows, selected_row))
            .ok();

        if event::poll(std::time::Duration::from_millis(250)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match map_key_event(key) {
                    DashboardAction::Quit => should_quit = true,
                    DashboardAction::CycleGroupBy => group_by = next_group_by(group_by),
                    DashboardAction::SelectNext => {
                        selected_row = move_selection(&rows, selected_row, 1);
                    }
                    DashboardAction::SelectPrev => {
                        selected_row = move_selection(&rows, selected_row, -1);
                    }
                    DashboardAction::Attach => {
                        if let Some(row) = rows.get(selected_row) {
                            if let Some(s) = &row.session {
                                attach_name = Some(s.name.clone());
                            }
                        }
                    }
                    DashboardAction::Spawn => spawn_requested = true,
                    DashboardAction::None => {}
                }
            }
        }
    }

    // Drop guard restores terminal before attach/spawn skim.
    drop(terminal);
    drop(_guard);

    if let Some(name) = attach_name {
        return crate::tmux_attach_session(&name);
    }
    if spawn_requested {
        return crate::cmd_spawn_interactive(&cfg);
    }
    ExitCode::SUCCESS
}

fn draw_dashboard(f: &mut Frame, grouped: &GroupedSessions, rows: &[FlatRow], selected_row: usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(f.area());

    let title = Paragraph::new(Line::from(vec![
        Span::styled("cdamux", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" — sessions"),
        Span::raw(format!("  group_by: {:?}", grouped.group_by)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = if rows.is_empty() {
        vec![ListItem::new("(no tmux sessions)")]
    } else {
        rows.iter()
            .enumerate()
            .map(|(i, row)| {
                if row.is_header {
                    ListItem::new(Line::from(Span::styled(
                        format!("## {}", row.label),
                        Style::default().fg(Color::Cyan),
                    )))
                } else if let Some(s) = &row.session {
                    let selected = i == selected_row;
                    let style = if selected {
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{}  ", s.name), style),
                        Span::raw(s.path.clone()),
                        Span::raw("  "),
                        Span::styled(
                            state_label(s.state),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]))
                } else {
                    ListItem::new("")
                }
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE)
            .title("sessions"),
    );
    f.render_widget(list, chunks[1]);

    let help = Paragraph::new(
        "↑/k ↓/j select · Enter attach · n spawn · g group · q quit",
    );
    f.render_widget(help, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn map_quit_and_nav() {
        assert_eq!(map_key_event(key(KeyCode::Char('q'))), DashboardAction::Quit);
        assert_eq!(map_key_event(key(KeyCode::Char('j'))), DashboardAction::SelectNext);
        assert_eq!(map_key_event(key(KeyCode::Enter)), DashboardAction::Attach);
        assert_eq!(map_key_event(key(KeyCode::Char('g'))), DashboardAction::CycleGroupBy);
    }

    #[test]
    fn group_by_cycles() {
        assert_eq!(next_group_by(GroupBy::Project), GroupBy::Cwd);
        assert_eq!(next_group_by(GroupBy::None), GroupBy::Project);
    }

    #[test]
    fn move_selection_skips_headers() {
        let grouped = GroupedSessions {
            group_by: GroupBy::Project,
            groups: vec![cdamux_core::SessionGroup {
                key: "a".into(),
                sessions: vec![EnrichedSession {
                    name: "c-one".into(),
                    path: "/p".into(),
                    group_key: "a".into(),
                    state: SessionState::Idle,
                }],
            }],
        };
        let rows = flatten_grouped(&grouped);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].is_header);
        let idx = session_indices(&rows);
        assert_eq!(idx, vec![1]);
        assert_eq!(move_selection(&rows, 1, 1), 1);
    }
}
