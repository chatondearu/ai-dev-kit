//! Full-screen ratatui dashboard (default `cdamux` with no subcommand).

use crate::app_tabs::{
    draw_agents, draw_projects, draw_sessions_list, draw_settings, draw_tab_bar, flat_sessions,
    help_for_tab, try_save, AppTab, bump_preview_lines, cycle_attach_mode, cycle_group_by,
};
use cdamux_core::{
    build_project_session_tree, capture_pane_text, config_path, discover_projects, load_config,
    project_key, send_to_session, state_icon, Config, EnrichedSession, ProjectSessionTree,
    SessionState,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashSet;
use std::io::{self, stdout, Stdout};
use std::process::ExitCode;

const COMPOSER_LABEL: &str = "send";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusArea {
    Tree,
    Composer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideTab {
    Preview,
    Notes,
    Review,
}

impl SideTab {
    fn next(self) -> Self {
        match self {
            SideTab::Preview => SideTab::Notes,
            SideTab::Notes => SideTab::Review,
            SideTab::Review => SideTab::Preview,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SideTab::Preview => "Preview",
            SideTab::Notes => "Notes",
            SideTab::Review => "Review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    None,
    Quit,
    Attach,
    Spawn,
    SelectNext,
    SelectPrev,
    ToggleExpand,
    FocusComposer,
    CycleSideTab,
    StartReview,
}

pub fn map_key_event(key: KeyEvent) -> DashboardAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return DashboardAction::Quit;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        return DashboardAction::StartReview;
    }
    match key.code {
        KeyCode::Char('q') => DashboardAction::Quit,
        KeyCode::Esc => DashboardAction::None,
        KeyCode::Char('n') | KeyCode::Char('s') => DashboardAction::Spawn,
        KeyCode::Char(' ') => DashboardAction::FocusComposer,
        KeyCode::Tab => DashboardAction::CycleSideTab,
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

fn session_at_row(rows: &[TreeRow], idx: usize) -> Option<&EnrichedSession> {
    match rows.get(idx)? {
        TreeRow::Session(s) | TreeRow::Unassigned(s) => Some(s),
        _ => None,
    }
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

fn fetch_preview(session_name: &str, max_lines: u32) -> String {
    let lines = max_lines.clamp(10, 500) as i32;
    match capture_pane_text(session_name, lines) {
        Ok(t) if t.trim().is_empty() => "(empty pane)".to_string(),
        Ok(t) => t,
        Err(_) => "(preview unavailable)".to_string(),
    }
}

fn sidecar_body(tab: SideTab, preview: &str, notes: &str, review: &str) -> String {
    match tab {
        SideTab::Preview => preview.to_string(),
        SideTab::Notes => notes.to_string(),
        SideTab::Review => review.to_string(),
    }
}

fn review_sidecar_hint(session: Option<&EnrichedSession>) -> String {
    match session {
        None => "Select a session.\n\nCtrl+R or Enter: open hunk review TUI.".to_string(),
        Some(s) => format!(
            "Session: {}\nRepo: {}\n\nCtrl+R or Enter: open hunk review (git diff → send to session).",
            s.name,
            s.path
        ),
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

struct DrawState<'a> {
    rows: &'a [TreeRow],
    selected_row: usize,
    side_tab: SideTab,
    focus: FocusArea,
    preview_text: &'a str,
    notes_text: &'a str,
    review_text: &'a str,
    composer_line: &'a str,
    composer_session: Option<&'a str>,
    status_line: &'a str,
}

fn row_index_for_session(rows: &[TreeRow], name: &str) -> Option<usize> {
    rows.iter().position(|r| match r {
        TreeRow::Session(s) | TreeRow::Unassigned(s) => s.name == name,
        _ => false,
    })
}

fn focus_session_in_tree(
    tree: &ProjectSessionTree,
    expanded: &mut HashSet<String>,
    name: &str,
) -> usize {
    for node in &tree.nodes {
        if node.sessions.iter().any(|s| s.name == name) {
            expanded.insert(project_key(&node.project.path));
            let rows = flatten_tree(tree, expanded);
            if let Some(idx) = row_index_for_session(&rows, name) {
                return idx;
            }
        }
    }
    if tree.unassigned.iter().any(|s| s.name == name) {
        let rows = flatten_tree(tree, expanded);
        return row_index_for_session(&rows, name).unwrap_or(0);
    }
    0
}

pub fn run_dashboard() -> ExitCode {
    let mut cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let mut app_tab = AppTab::Fleet;
    let mut projects_scroll = 0usize;
    let mut sessions_selected = 0usize;
    let mut sessions_scroll = 0usize;
    let mut settings_dirty = false;
    let mut settings_status = String::new();
    let config_path_display = config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unknown)".into());

    let tree = build_project_session_tree(&cfg.projects, cfg.daemon.idle_seconds_for_needs_input);
    let mut expanded = default_expanded(&tree);
    let mut selected_row = 0;
    let mut side_tab = SideTab::Preview;
    let mut focus = FocusArea::Tree;
    let mut composer_line = String::new();
    let mut preview_text = String::new();
    let mut preview_for_session: Option<String> = None;
    let mut notes_text = String::new();
    let mut notes_for_session: Option<String> = None;
    let mut status_line = String::new();

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
    let mut review_session: Option<String> = None;

    while !should_quit && attach_name.is_none() && !spawn_requested && review_session.is_none() {
        let tree = build_project_session_tree(&cfg.projects, cfg.daemon.idle_seconds_for_needs_input);
        let rows = flatten_tree(&tree, &expanded);
        selected_row = clamp_selection(&rows, selected_row);

        let discovered = discover_projects(&cfg.projects);
        let flat = flat_sessions(&cfg);
        if sessions_selected >= flat.len() && !flat.is_empty() {
            sessions_selected = flat.len() - 1;
        }
        let agent_lines = crate::doctor_agent_lines(&cfg);

        let preview_max = cfg.ui.preview_max_lines;
        let session = session_at_row(&rows, selected_row);
        if let Some(s) = session {
            if preview_for_session.as_deref() != Some(s.name.as_str()) {
                preview_text = fetch_preview(&s.name, preview_max);
                preview_for_session = Some(s.name.clone());
            }
            if notes_for_session.as_deref() != Some(s.name.as_str()) {
                notes_text = crate::note_text_for_session_cwd(&cfg, &s.path);
                notes_for_session = Some(s.name.clone());
            }
        } else {
            preview_for_session = None;
            preview_text = "(select a session for preview)".to_string();
            notes_for_session = None;
            notes_text = "Select a session to view project notes.".to_string();
        }
        let review_text = review_sidecar_hint(session);

        let composer_session = session.map(|s| s.name.as_str());

        let help_status = if app_tab == AppTab::Settings {
            if settings_status.is_empty() {
                status_line.as_str()
            } else {
                settings_status.as_str()
            }
        } else {
            status_line.as_str()
        };

        terminal
            .draw(|f| {
                draw_app_screen(
                    f,
                    app_tab,
                    DrawState {
                        rows: &rows,
                        selected_row,
                        side_tab,
                        focus,
                        preview_text: &preview_text,
                        notes_text: &notes_text,
                        review_text: &review_text,
                        composer_line: &composer_line,
                        composer_session,
                        status_line: help_status,
                    },
                    &cfg,
                    settings_dirty,
                    &config_path_display,
                    &discovered,
                    projects_scroll,
                    &flat,
                    sessions_selected,
                    sessions_scroll,
                    &agent_lines,
                );
            })
            .ok();

        if event::poll(std::time::Duration::from_millis(250)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if let KeyCode::Char(c) = key.code {
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                        if let Some(tab) = AppTab::from_digit(c) {
                            app_tab = tab;
                            continue;
                        }
                    }
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    should_quit = true;
                    continue;
                }
                if key.code == KeyCode::Char('q')
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                {
                    should_quit = true;
                    continue;
                }

                if app_tab == AppTab::Settings {
                    if handle_settings_keys(&mut cfg, key, &mut settings_dirty, &mut settings_status) {
                        continue;
                    }
                } else if app_tab == AppTab::Projects {
                    if handle_projects_keys(key, &discovered, &mut projects_scroll) {
                        continue;
                    }
                } else if app_tab == AppTab::Sessions {
                    if handle_sessions_keys(
                        key,
                        &flat,
                        &mut sessions_selected,
                        &mut sessions_scroll,
                        &tree,
                        &mut expanded,
                        &mut selected_row,
                        &mut app_tab,
                    ) {
                        continue;
                    }
                } else if app_tab != AppTab::Fleet {
                    continue;
                }

                if focus == FocusArea::Composer {
                    if key.code == KeyCode::Esc {
                        focus = FocusArea::Tree;
                        continue;
                    }
                    if key.code == KeyCode::Enter {
                        if let Some(name) = composer_session {
                            let text = composer_line.clone();
                            match send_to_session(name, &text, true) {
                                Ok(()) => {
                                    status_line = format!("sent to {}", name);
                                    composer_line.clear();
                                    preview_text = fetch_preview(name, preview_max);
                                    preview_for_session = Some(name.to_string());
                                }
                                Err(e) => status_line = e.to_string(),
                            }
                        }
                        focus = FocusArea::Tree;
                        continue;
                    }
                    if key.code == KeyCode::Backspace {
                        composer_line.pop();
                        continue;
                    }
                    if let KeyCode::Char(c) = key.code {
                        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                            composer_line.push(c);
                        }
                        continue;
                    }
                    continue;
                }

                match map_key_event(key) {
                    DashboardAction::Quit => should_quit = true,
                    DashboardAction::CycleSideTab => side_tab = side_tab.next(),
                    DashboardAction::FocusComposer => {
                        if side_tab == SideTab::Preview && composer_session.is_some() {
                            focus = FocusArea::Composer;
                        }
                    }
                    DashboardAction::SelectNext => {
                        selected_row = move_selection(&rows, selected_row, 1);
                        preview_for_session = None;
                        notes_for_session = None;
                    }
                    DashboardAction::SelectPrev => {
                        selected_row = move_selection(&rows, selected_row, -1);
                        preview_for_session = None;
                        notes_for_session = None;
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
                    DashboardAction::StartReview => {
                        if let Some(s) = session_at_row(&rows, selected_row) {
                            review_session = Some(s.name.clone());
                        } else {
                            status_line = "select a session for review".into();
                        }
                    }
                    DashboardAction::Attach => {
                        if side_tab == SideTab::Review {
                            if let Some(s) = session_at_row(&rows, selected_row) {
                                review_session = Some(s.name.clone());
                            }
                            continue;
                        }
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
        return crate::tmux_attach_session_with_mode(&name, Some(cfg.sessions.attach_mode));
    }
    if spawn_requested {
        return crate::spawn_modal::run(&cfg);
    }
    if let Some(name) = review_session {
        return crate::run_review_for_session(&name);
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

fn handle_settings_keys(
    cfg: &mut Config,
    key: KeyEvent,
    dirty: &mut bool,
    status: &mut String,
) -> bool {
    match key.code {
        KeyCode::Char('a') if key.modifiers.is_empty() => {
            cfg.sessions.attach_mode = cycle_attach_mode(cfg.sessions.attach_mode);
            *dirty = true;
            status.clear();
            true
        }
        KeyCode::Char('g') if key.modifiers.is_empty() => {
            cfg.sessions.group_by = cycle_group_by(cfg.sessions.group_by);
            *dirty = true;
            status.clear();
            true
        }
        KeyCode::Char('+') | KeyCode::Char('=') if key.modifiers.is_empty() => {
            cfg.ui.preview_max_lines = bump_preview_lines(cfg.ui.preview_max_lines, 1);
            *dirty = true;
            status.clear();
            true
        }
        KeyCode::Char('-') if key.modifiers.is_empty() => {
            cfg.ui.preview_max_lines = bump_preview_lines(cfg.ui.preview_max_lines, -1);
            *dirty = true;
            status.clear();
            true
        }
        KeyCode::Char('t') if key.modifiers.is_empty() => {
            cfg.daemon.listen_tailnet = !cfg.daemon.listen_tailnet;
            *dirty = true;
            status.clear();
            true
        }
        KeyCode::Char('s') | KeyCode::Char('S') if key.modifiers.is_empty() => {
            match try_save(cfg) {
                Ok(msg) => {
                    *status = msg;
                    *dirty = false;
                }
                Err(e) => *status = e,
            }
            true
        }
        _ => false,
    }
}

fn handle_projects_keys(key: KeyEvent, discovered: &[cdamux_core::Project], scroll: &mut usize) -> bool {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if *scroll + 1 < discovered.len() {
                *scroll += 1;
            }
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            *scroll = scroll.saturating_sub(1);
            true
        }
        _ => false,
    }
}

fn handle_sessions_keys(
    key: KeyEvent,
    flat: &[EnrichedSession],
    selected: &mut usize,
    scroll: &mut usize,
    tree: &ProjectSessionTree,
    expanded: &mut HashSet<String>,
    fleet_row: &mut usize,
    app_tab: &mut AppTab,
) -> bool {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if flat.is_empty() {
                return true;
            }
            if *selected + 1 < flat.len() {
                *selected += 1;
                if *selected >= *scroll + 1 {
                    *scroll = *selected;
                }
            }
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
                if *selected < *scroll {
                    *scroll = *selected;
                }
            }
            true
        }
        KeyCode::Enter => {
            if let Some(s) = flat.get(*selected) {
                *fleet_row = focus_session_in_tree(tree, expanded, &s.name);
                *app_tab = AppTab::Fleet;
            }
            true
        }
        _ => false,
    }
}

fn draw_app_screen(
    f: &mut Frame,
    app_tab: AppTab,
    state: DrawState<'_>,
    cfg: &Config,
    settings_dirty: bool,
    config_path: &str,
    discovered: &[cdamux_core::Project],
    projects_scroll: usize,
    flat_sessions: &[EnrichedSession],
    sessions_selected: usize,
    sessions_scroll: usize,
    agent_lines: &[String],
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
        AppTab::Fleet => draw_fleet_body(f, root[1], &state),
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
        AppTab::Settings => draw_settings(
            f,
            root[1],
            cfg,
            settings_dirty,
            state.status_line,
            config_path,
        ),
    }

    let help = if app_tab == AppTab::Fleet && state.focus == FocusArea::Composer {
        "composer: type · Enter send+↵ · Esc back"
    } else {
        help_for_tab(app_tab, settings_dirty)
    };
    let help_line = if state.status_line.is_empty() {
        help.to_string()
    } else {
        format!("{}  |  {}", help, state.status_line)
    };
    f.render_widget(Paragraph::new(help_line), root[2]);
}

fn draw_fleet_body(f: &mut Frame, area: ratatui::layout::Rect, state: &DrawState<'_>) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4)])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("cdamux", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" — fleet"),
        Span::styled("  ○ idle ● active ◆ needs", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title, inner[0]);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(inner[1]);

    draw_tree_column(f, columns[0], state.rows, state.selected_row);
    draw_sidecar(f, columns[1], state);
}

fn draw_tree_column(f: &mut Frame, area: ratatui::layout::Rect, rows: &[TreeRow], selected_row: usize) {
    let items: Vec<ListItem> = if rows.is_empty() {
        vec![ListItem::new("(no projects or sessions)")]
    } else {
        rows.iter()
            .enumerate()
            .map(|(i, row)| tree_row_item(row, i == selected_row))
            .collect()
    };
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::RIGHT)
            .title("projects → sessions"),
    );
    f.render_widget(list, area);
}

fn tree_row_item(row: &TreeRow, selected: bool) -> ListItem<'_> {
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
}

fn draw_sidecar(f: &mut Frame, area: ratatui::layout::Rect, state: &DrawState) {
    let show_composer = state.side_tab == SideTab::Preview;
    let vertical = if show_composer {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(4),
                Constraint::Length(3),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(4)])
            .split(area)
    };

    let tabs = Line::from(
        [SideTab::Preview, SideTab::Notes, SideTab::Review]
            .iter()
            .map(|t| {
                let style = if *t == state.side_tab {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Span::styled(format!(" {} ", t.label()), style)
            })
            .collect::<Vec<_>>(),
    );
    f.render_widget(Paragraph::new(tabs), vertical[0]);

    let body = sidecar_body(
        state.side_tab,
        state.preview_text,
        state.notes_text,
        state.review_text,
    );
    let preview = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::NONE).title(" "));
    f.render_widget(preview, vertical[1]);

    if show_composer {
        let target = state
            .composer_session
            .map(|n| format!("→ {}", n))
            .unwrap_or_else(|| "→ (no session)".to_string());
        let border_color = if state.focus == FocusArea::Composer {
            Color::Cyan
        } else {
            Color::DarkGray
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
        f.render_widget(composer, vertical[2]);
    }
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
        assert_eq!(map_key_event(key(KeyCode::Char(' '))), DashboardAction::FocusComposer);
        assert_eq!(map_key_event(key(KeyCode::Tab)), DashboardAction::CycleSideTab);
    }

    #[test]
    fn map_ctrl_r_review() {
        let key = KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(map_key_event(key), DashboardAction::StartReview);
    }

    #[test]
    fn side_tab_cycles() {
        assert_eq!(SideTab::Preview.next(), SideTab::Notes);
        assert_eq!(SideTab::Review.next(), SideTab::Preview);
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
