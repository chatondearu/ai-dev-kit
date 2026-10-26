//! Full-screen ratatui dashboard (default `cdamux` with no subcommand).

use cdamux_core::{
    build_project_session_tree, load_config, project_key, state_icon, EnrichedSession,
    ProjectSessionTree, SessionState,
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
use std::collections::HashSet;
use std::io::{self, stdout, Stdout};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    None,
    Quit,
    Attach,
    Spawn,
    SelectNext,
    SelectPrev,
    ToggleExpand,
}

pub fn map_key_event(key: KeyEvent) -> DashboardAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return DashboardAction::Quit;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => DashboardAction::Quit,
        KeyCode::Char('n') | KeyCode::Char('s') => DashboardAction::Spawn,
        KeyCode::Enter => DashboardAction::Attach,
        KeyCode::Down | KeyCode::Char('j') => DashboardAction::SelectNext,
        KeyCode::Up | KeyCode::Char('k') => DashboardAction::SelectPrev,
        KeyCode::Right | KeyCode::Char('l') => DashboardAction::ToggleExpand,
        KeyCode::Left | KeyCode::Char('h') => DashboardAction::ToggleExpand,
        _ => DashboardAction::None,
    }
}

#[derive(Debug, Clone)]
enum TreeRow {
    Project {
        key: String,
        name: String,
        path: String,
        session_count: usize,
        expanded: bool,
    },
    Session(EnrichedSession),
    UnassignedHeader,
    Unassigned(EnrichedSession),
}

fn flatten_tree(tree: &ProjectSessionTree, expanded: &HashSet<String>) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for node in &tree.nodes {
        let key = project_key(&node.project.path);
        let is_exp = expanded.contains(&key);
        rows.push(TreeRow::Project {
            key,
            name: node.project.name.clone(),
            path: node.project.path.to_string_lossy().to_string(),
            session_count: node.sessions.len(),
            expanded: is_exp,
        });
        if is_exp {
            for s in &node.sessions {
                rows.push(TreeRow::Session(s.clone()));
            }
        }
    }
    if !tree.unassigned.is_empty() {
        rows.push(TreeRow::UnassignedHeader);
        for s in &tree.unassigned {
            rows.push(TreeRow::Unassigned(s.clone()));
        }
    }
    rows
}

fn selectable_indices(rows: &[TreeRow]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, r)| {
            matches!(
                r,
                TreeRow::Project { .. } | TreeRow::Session(_) | TreeRow::Unassigned(_)
            )
        })
        .map(|(i, _)| i)
        .collect()
}

fn clamp_selection(rows: &[TreeRow], selected_row: usize) -> usize {
    let indices = selectable_indices(rows);
    if indices.is_empty() {
        return 0;
    }
    if indices.contains(&selected_row) {
        return selected_row;
    }
    indices[0]
}

fn move_selection(rows: &[TreeRow], selected_row: usize, delta: i32) -> usize {
    let indices = selectable_indices(rows);
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

fn default_expanded(tree: &ProjectSessionTree) -> HashSet<String> {
    tree.nodes
        .iter()
        .filter(|n| !n.sessions.is_empty())
        .map(|n| project_key(&n.project.path))
        .collect()
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

    let tree = build_project_session_tree(&cfg.projects, cfg.daemon.idle_seconds_for_needs_input);
    let mut expanded = default_expanded(&tree);
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
        let tree = build_project_session_tree(&cfg.projects, cfg.daemon.idle_seconds_for_needs_input);
        let rows = flatten_tree(&tree, &expanded);
        selected_row = clamp_selection(&rows, selected_row);

        terminal
            .draw(|f| draw_dashboard(f, &rows, selected_row))
            .ok();

        if event::poll(std::time::Duration::from_millis(250)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match map_key_event(key) {
                    DashboardAction::Quit => should_quit = true,
                    DashboardAction::SelectNext => {
                        selected_row = move_selection(&rows, selected_row, 1);
                    }
                    DashboardAction::SelectPrev => {
                        selected_row = move_selection(&rows, selected_row, -1);
                    }
                    DashboardAction::ToggleExpand => {
                        if let Some(TreeRow::Project { key, expanded: is_exp, .. }) =
                            rows.get(selected_row)
                        {
                            if *is_exp {
                                expanded.remove(key);
                            } else {
                                expanded.insert(key.clone());
                            }
                        }
                    }
                    DashboardAction::Attach => {
                        match rows.get(selected_row) {
                            Some(TreeRow::Session(s) | TreeRow::Unassigned(s)) => {
                                attach_name = Some(s.name.clone());
                            }
                            Some(TreeRow::Project { key, expanded: is_exp, .. }) => {
                                if *is_exp {
                                    expanded.remove(key);
                                } else {
                                    expanded.insert(key.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    DashboardAction::Spawn => spawn_requested = true,
                    DashboardAction::None => {}
                }
            }
        }
    }

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

fn session_state_color(state: SessionState) -> Color {
    match state {
        SessionState::Idle => Color::DarkGray,
        SessionState::Active => Color::Green,
        SessionState::NeedsInput => Color::Yellow,
    }
}

fn draw_dashboard(f: &mut Frame, rows: &[TreeRow], selected_row: usize) {
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
        Span::raw(" — fleet"),
        Span::styled("  ○ idle ● active ◆ needs", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = if rows.is_empty() {
        vec![ListItem::new("(no projects or sessions)")]
    } else {
        rows.iter()
            .enumerate()
            .map(|(i, row)| {
                let selected = i == selected_row;
                let sel_style = if selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                match row {
                    TreeRow::Project {
                        name,
                        path,
                        session_count,
                        expanded,
                        ..
                    } => {
                        let chevron = if *expanded { "▼" } else { "▶" };
                        ListItem::new(Line::from(vec![
                            Span::raw(format!("{} ", chevron)),
                            Span::styled("◇ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                format!("{} ", name),
                                sel_style.fg(Color::Cyan).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!("({}) ", session_count),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(path.clone(), Style::default().fg(Color::DarkGray)),
                        ]))
                    }
                    TreeRow::Session(s) | TreeRow::Unassigned(s) => {
                        let icon = state_icon(s.state);
                        ListItem::new(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(
                                format!("{} ", icon),
                                Style::default().fg(session_state_color(s.state)),
                            ),
                            Span::styled(format!("{}  ", s.name), sel_style),
                            Span::styled(s.path.clone(), Style::default().fg(Color::DarkGray)),
                        ]))
                    }
                    TreeRow::UnassignedHeader => ListItem::new(Line::from(Span::styled(
                        "— unassigned sessions",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::ITALIC),
                    ))),
                }
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::NONE)
            .title("projects → sessions"),
    );
    f.render_widget(list, chunks[1]);

    let help = Paragraph::new(
        "↑/k ↓/j move · Enter attach / toggle project · h/l fold · n spawn · q quit",
    );
    f.render_widget(help, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdamux_core::{Project, SessionState};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use std::path::PathBuf;

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
        assert_eq!(map_key_event(key(KeyCode::Right)), DashboardAction::ToggleExpand);
    }

    #[test]
    fn flatten_respects_expand() {
        let tree = ProjectSessionTree {
            nodes: vec![cdamux_core::ProjectTreeNode {
                project: Project {
                    name: "p".into(),
                    path: PathBuf::from("/tmp/p"),
                    root_label: "dev".into(),
                },
                sessions: vec![cdamux_core::EnrichedSession {
                    name: "s1".into(),
                    path: "/tmp/p".into(),
                    group_key: "p".into(),
                    state: SessionState::Idle,
                }],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = HashSet::new();
        let collapsed = flatten_tree(&tree, &exp);
        assert_eq!(collapsed.len(), 1);
        let exp2: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let open = flatten_tree(&tree, &exp2);
        assert_eq!(open.len(), 2);
    }
}
