//! Full-screen ratatui dashboard (default `cdamux` with no subcommand).

use ansi_to_tui::IntoText as _;
use crate::spawn_modal::{
    draw_modal, draw_overlay_scrim, on_key as spawn_modal_on_key, ModalState, SpawnModalAction,
};
use crate::app_tabs::{
    draw_agents, draw_projects, draw_sessions_list, draw_tab_bar, flat_sessions, help_for_tab,
    AppTab, cycle_group_by,
};
use crate::keymap::{fleet_help_text, map_key_event, DashboardAction};
use crate::settings_form::{draw_settings, SettingsForm};
use crate::widgets::edit_text;
use cdamux_core::{
    build_project_session_tree, capture_pane_text, config_path, discover_projects,
    format_orch_graph_panel, kill_tmux_session, load_config,
    preferred_spawn_agent, project_key, rename_tmux_session, resolve_agent_command,
    send_to_session, session_name_for_path, spawn_tmux_session_with_layout, state_icon,
    switch_client_to_session, unique_session_name, Config, EnrichedSession, GroupBy, SessionChrome,
    PaneLayout, ProjectSessionTree, ProjectTreeNode, SessionState, SpawnOutcome,
};
use std::path::PathBuf;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEventKind,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap},
    Frame, Terminal,
};
use std::collections::HashSet;
use std::io::{self, stdout, Stdout};
use std::process::ExitCode;
use std::time::{Duration, Instant};

const COMPOSER_LABEL: &str = "send";

/// Palette aligned with `prototype/dashboard-ui` (visible on dark terminals).
const CLR_TEXT: Color = Color::Rgb(230, 234, 240);
const CLR_MUTED: Color = Color::Rgb(139, 149, 168);
const CLR_ACCENT: Color = Color::Rgb(110, 181, 255);
const CLR_PROJECT: Color = Color::Rgb(158, 200, 255);
const CLR_OK: Color = Color::Rgb(107, 207, 142);
const CLR_WARN: Color = Color::Rgb(232, 184, 74);
const CLR_IDLE: Color = Color::Rgb(92, 101, 120);
const CLR_SEL_BG: Color = Color::Rgb(42, 49, 64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusArea {
    Tree,
    Composer,
    PreviewScroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideTab {
    Preview,
    Notes,
    Review,
    OrchGraph,
}

impl SideTab {
    fn next(self) -> Self {
        match self {
            SideTab::Preview => SideTab::Notes,
            SideTab::Notes => SideTab::Review,
            SideTab::Review => SideTab::OrchGraph,
            SideTab::OrchGraph => SideTab::Preview,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SideTab::Preview => "Preview",
            SideTab::Notes => "Notes",
            SideTab::Review => "Review",
            SideTab::OrchGraph => "Orch graph",
        }
    }
}

struct RenameState {
    old_name: String,
    input: String,
}

enum RenameAction {
    Continue,
    Cancel,
    Confirm,
}

fn draw_rename_modal(f: &mut Frame, state: &RenameState) {
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

fn rename_on_key(state: &mut RenameState, key: KeyEvent) -> RenameAction {
    match key.code {
        KeyCode::Esc => RenameAction::Cancel,
        KeyCode::Enter => RenameAction::Confirm,
        _ => {
            // tmux session names: ASCII only.
            edit_text(&mut state.input, key, |c| c.is_ascii());
            RenameAction::Continue
        }
    }
}

fn fork_selected_session(cfg: &Config, s: &EnrichedSession) -> Result<String, String> {
    let path = PathBuf::from(&s.path);
    if s.path.is_empty() || !path.is_dir() {
        return Err("session has no project directory to fork".into());
    }
    let base = unique_session_name(&session_name_for_path(&path));
    let agent = preferred_spawn_agent(&cfg.agents);
    let cmd = resolve_agent_command(&cfg.agents, &agent).map_err(|e| e.to_string())?;
    match spawn_tmux_session_with_layout(
        &base,
        &path,
        &cmd,
        PaneLayout::Single,
        cfg.sessions.attach_mode,
        false,
        Some(agent.as_str()),
    ) {
        Ok(SpawnOutcome::Created) => Ok(format!("forked '{}' with {}", base, agent)),
        Ok(SpawnOutcome::ReusedExisting) => Ok(format!("reusing '{}'", base)),
        Err(e) => Err(e.to_string()),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn tree_node_key(node: &ProjectTreeNode, group_by: GroupBy) -> String {
    if group_by == GroupBy::Project {
        project_key(&node.project.path)
    } else {
        format!("grp:{}", node.project.name)
    }
}

fn session_passes_waiting_filter(session: &EnrichedSession, waiting_only: bool) -> bool {
    !waiting_only || session.state == SessionState::NeedsInput
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

fn flatten_tree(
    tree: &ProjectSessionTree,
    expanded: &HashSet<String>,
    group_by: GroupBy,
    waiting_only: bool,
) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for node in &tree.nodes {
        let visible: Vec<EnrichedSession> = node
            .sessions
            .iter()
            .filter(|s| session_passes_waiting_filter(s, waiting_only))
            .cloned()
            .collect();
        if waiting_only && visible.is_empty() {
            continue;
        }
        let key = tree_node_key(node, group_by);
        let is_exp = expanded.contains(&key);
        rows.push(TreeRow::Project {
            key,
            name: node.project.name.clone(),
            path: node.project.path.to_string_lossy().to_string(),
            session_count: visible.len(),
            expanded: is_exp,
        });
        if is_exp {
            for s in visible {
                rows.push(TreeRow::Session(s));
            }
        }
    }
    let unassigned: Vec<EnrichedSession> = tree
        .unassigned
        .iter()
        .filter(|s| session_passes_waiting_filter(s, waiting_only))
        .cloned()
        .collect();
    if !unassigned.is_empty() {
        rows.push(TreeRow::UnassignedHeader);
        for s in unassigned {
            rows.push(TreeRow::Unassigned(s));
        }
    }
    rows
}

fn toggle_fold_all(tree: &ProjectSessionTree, expanded: &mut HashSet<String>, group_by: GroupBy) {
    let keys: Vec<String> = tree
        .nodes
        .iter()
        .filter(|n| !n.sessions.is_empty())
        .map(|n| tree_node_key(n, group_by))
        .collect();
    let all_open = !keys.is_empty() && keys.iter().all(|k| expanded.contains(k));
    if all_open {
        expanded.clear();
    } else {
        for k in keys {
            expanded.insert(k);
        }
    }
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

fn default_expanded(tree: &ProjectSessionTree, group_by: GroupBy) -> HashSet<String> {
    tree.nodes
        .iter()
        .filter(|n| !n.sessions.is_empty())
        .map(|n| tree_node_key(n, group_by))
        .collect()
}

/// `scroll_from_bottom`: 0 = tail of capture; higher = scroll up into older lines.
/// `viewport_lines`: visible rows in the sidecar (show the newest lines that fit).
fn preview_display_lines(
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

fn fetch_preview(session_name: &str, max_lines: u32) -> String {
    let lines = max_lines.clamp(10, 500) as i32;
    match capture_pane_text(session_name, lines) {
        Ok(t) if t.trim().is_empty() => "(empty pane)".to_string(),
        Ok(t) => t,
        Err(_) => "(preview unavailable)".to_string(),
    }
}

fn sidecar_plain_body(tab: SideTab, notes: &str, review: &str, orch_graph: &str) -> String {
    match tab {
        SideTab::Preview => String::new(),
        SideTab::Notes => notes.to_string(),
        SideTab::Review => review.to_string(),
        SideTab::OrchGraph => orch_graph.to_string(),
    }
}

fn orch_graph_text_for_cwd(cwd: &str, width: usize) -> String {
    if cwd.is_empty() {
        return "Select a session tied to a repo running orch.sh.".to_string();
    }
    match cdamux_core::find_orch_graph_from_cwd(cwd) {
        Some(g) => format_orch_graph_panel(&g, width),
        None => "(no live graph — start orch.sh in this repo; tab cycles with sidecar)".to_string(),
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
        crossterm::execute!(
            stdout(),
            crossterm::terminal::EnterAlternateScreen,
            EnableMouseCapture,
        )?;
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        Ok((terminal, Self))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            stdout(),
            DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen,
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

struct DrawState<'a> {
    rows: &'a [TreeRow],
    selected_row: usize,
    side_tab: SideTab,
    focus: FocusArea,
    preview_text: &'a str,
    preview_scroll: usize,
    notes_text: &'a str,
    review_text: &'a str,
    orch_graph_text: &'a str,
    composer_line: &'a str,
    composer_session: Option<&'a str>,
    session_chrome: Option<&'a SessionChrome>,
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
    group_by: GroupBy,
    waiting_only: bool,
) -> usize {
    for node in &tree.nodes {
        if node.sessions.iter().any(|s| s.name == name) {
            expanded.insert(tree_node_key(node, group_by));
            let rows = flatten_tree(tree, expanded, group_by, waiting_only);
            if let Some(idx) = row_index_for_session(&rows, name) {
                return idx;
            }
        }
    }
    if tree.unassigned.iter().any(|s| s.name == name) {
        let rows = flatten_tree(tree, expanded, group_by, waiting_only);
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
    let mut settings = SettingsForm::new(config_path());
    let config_path_display = config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unknown)".into());

    let group_by = cfg.sessions.group_by;
    let tree = build_project_session_tree(
        &cfg.projects,
        cfg.daemon.idle_seconds_for_needs_input,
        group_by,
    );
    let mut expanded = default_expanded(&tree, group_by);
    let mut waiting_only = false;
    let mut show_help = false;
    let mut selected_row = 0;
    let mut side_tab = SideTab::Preview;
    let mut focus = FocusArea::Tree;
    let mut composer_line = String::new();
    let mut preview_text = String::new();
    let mut preview_for_session: Option<String> = None;
    let mut session_chrome = SessionChrome::gather("", "");
    let mut chrome_for_session: Option<String> = None;
    let mut notes_text = String::new();
    let mut notes_for_session: Option<String> = None;
    let mut orch_graph_text = String::new();
    let mut orch_graph_for_session: Option<String> = None;
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
    let mut spawn_overlay: Option<ModalState> = None;
    let mut rename_overlay: Option<RenameState> = None;
    let mut review_session: Option<String> = None;
    let mut fleet_tree_rect: Option<ratatui::layout::Rect> = None;
    let mut sidecar_preview_rect: Option<ratatui::layout::Rect> = None;
    let mut preview_scroll = 0usize;
    let mut cached_tree = tree;
    let mut tree_dirty = true;
    let mut last_tree_refresh = Instant::now();
    let mut last_preview_refresh = Instant::now();
    const TREE_REFRESH: Duration = Duration::from_millis(2000);
    const PREVIEW_REFRESH: Duration = Duration::from_millis(800);

    while !should_quit && attach_name.is_none() && review_session.is_none() {
        let group_by = cfg.sessions.group_by;
        if tree_dirty || last_tree_refresh.elapsed() >= TREE_REFRESH {
            cached_tree = build_project_session_tree(
                &cfg.projects,
                cfg.daemon.idle_seconds_for_needs_input,
                group_by,
            );
            last_tree_refresh = Instant::now();
            tree_dirty = false;
        }
        let tree = &cached_tree;
        let rows = flatten_tree(tree, &expanded, group_by, waiting_only);
        selected_row = clamp_selection(&rows, selected_row);

        let discovered = if app_tab == AppTab::Projects {
            discover_projects(&cfg.projects)
        } else {
            Vec::new()
        };
        let flat = if app_tab == AppTab::Sessions {
            flat_sessions(&cfg)
        } else {
            Vec::new()
        };
        if app_tab == AppTab::Sessions && sessions_selected >= flat.len() && !flat.is_empty() {
            sessions_selected = flat.len() - 1;
        }
        let agent_lines = if app_tab == AppTab::Agents {
            crate::doctor_agent_lines(&cfg)
        } else {
            Vec::new()
        };

        let preview_max = cfg.ui.preview_max_lines;
        let session = session_at_row(&rows, selected_row);
        if let Some(s) = session {
            let session_changed = preview_for_session.as_deref() != Some(s.name.as_str());
            if session_changed {
                preview_scroll = 0;
                preview_text = fetch_preview(&s.name, preview_max);
                preview_for_session = Some(s.name.clone());
                session_chrome = SessionChrome::gather(&s.name, &s.path);
                chrome_for_session = Some(s.name.clone());
                last_preview_refresh = Instant::now();
            } else if side_tab == SideTab::Preview
                && last_preview_refresh.elapsed() >= PREVIEW_REFRESH
            {
                preview_text = fetch_preview(&s.name, preview_max);
                session_chrome.refresh_volatile(&s.name);
                last_preview_refresh = Instant::now();
            }
            if notes_for_session.as_deref() != Some(s.name.as_str()) {
                notes_text = crate::note_text_for_session_cwd(&cfg, &s.path);
                notes_for_session = Some(s.name.clone());
            }
            let graph_key = format!("{}:{}", s.name, s.path);
            if orch_graph_for_session.as_deref() != Some(graph_key.as_str())
                || (side_tab == SideTab::OrchGraph
                    && last_preview_refresh.elapsed() >= PREVIEW_REFRESH)
            {
                orch_graph_text = orch_graph_text_for_cwd(&s.path, 72);
                orch_graph_for_session = Some(graph_key);
            }
        } else {
            preview_for_session = None;
            chrome_for_session = None;
            preview_scroll = 0;
            preview_text = "(select a session for preview)".to_string();
            session_chrome = SessionChrome::gather("", "");
            notes_for_session = None;
            notes_text = "Select a session to view project notes.".to_string();
            orch_graph_for_session = None;
            orch_graph_text = orch_graph_text_for_cwd("", 72);
        }
        let review_text = review_sidecar_hint(session);

        let composer_session = session.map(|s| s.name.as_str());

        let help_status = if app_tab == AppTab::Settings {
            if settings.status.is_empty() {
                status_line.as_str()
            } else {
                settings.status.as_str()
            }
        } else {
            status_line.as_str()
        };

        let modal_open = spawn_overlay.is_some() || rename_overlay.is_some();
        terminal
            .draw(|f| {
                if modal_open {
                    draw_overlay_scrim(f);
                    if let Some(overlay) = &spawn_overlay {
                        draw_modal(f, overlay);
                    }
                    if let Some(overlay) = &rename_overlay {
                        draw_rename_modal(f, overlay);
                    }
                } else {
                    draw_app_screen(
                        f,
                        app_tab,
                        DrawState {
                            rows: &rows,
                            selected_row,
                            side_tab,
                            focus,
                            preview_text: &preview_text,
                            preview_scroll,
                            notes_text: &notes_text,
                            review_text: &review_text,
                            orch_graph_text: &orch_graph_text,
                            composer_line: &composer_line,
                            composer_session,
                            session_chrome: if chrome_for_session.is_some() {
                                Some(&session_chrome)
                            } else {
                                None
                            },
                            status_line: help_status,
                        },
                        &cfg,
                        &settings,
                        &config_path_display,
                        &discovered,
                        projects_scroll,
                        &flat,
                        sessions_selected,
                        sessions_scroll,
                        &agent_lines,
                        waiting_only,
                        show_help,
                        &mut fleet_tree_rect,
                        &mut sidecar_preview_rect,
                    );
                }
            })
            .ok();

        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Mouse(m)) => {
                    if app_tab == AppTab::Fleet {
                        if focus == FocusArea::Tree
                            && m.kind == MouseEventKind::Down(MouseButton::Left)
                        {
                            if let Some(area) = fleet_tree_rect {
                                if let Some(idx) =
                                    row_index_from_mouse(area, m.column, m.row, rows.len())
                                {
                                    selected_row = clamp_selection(&rows, idx);
                                    preview_for_session = None;
                                    notes_for_session = None;
                                }
                            }
                        } else if side_tab == SideTab::Preview
                            && focus != FocusArea::Composer
                        {
                            if let Some(area) = sidecar_preview_rect {
                                if m.column >= area.x
                                    && m.column < area.x + area.width
                                    && m.row >= area.y
                                    && m.row < area.y + area.height
                                {
                                    match m.kind {
                                        MouseEventKind::ScrollUp => {
                                            preview_scroll = preview_scroll.saturating_sub(3);
                                        }
                                        MouseEventKind::ScrollDown => preview_scroll += 3,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
                Ok(Event::Key(key)) => {
                if let Some(ref mut overlay) = spawn_overlay {
                    match spawn_modal_on_key(&cfg, overlay, key) {
                        SpawnModalAction::Continue => continue,
                        SpawnModalAction::DismissCancel => {
                            spawn_overlay = None;
                            continue;
                        }
                        SpawnModalAction::DismissDone { status } => {
                            spawn_overlay = None;
                            tree_dirty = true;
                            status_line = status;
                            selected_row = 0;
                            continue;
                        }
                    }
                }
                if let Some(ref mut overlay) = rename_overlay {
                    match rename_on_key(overlay, key) {
                        RenameAction::Continue => continue,
                        RenameAction::Cancel => {
                            rename_overlay = None;
                            continue;
                        }
                        RenameAction::Confirm => {
                            let old = overlay.old_name.clone();
                            let new = overlay.input.trim().to_string();
                            rename_overlay = None;
                            if new.is_empty() {
                                status_line = "rename: empty name".into();
                            } else if new == old {
                                status_line = "rename: unchanged".into();
                            } else {
                                match rename_tmux_session(&old, &new) {
                                    Ok(()) => {
                                        tree_dirty = true;
                                        status_line = format!("renamed {} → {}", old, new);
                                    }
                                    Err(e) => status_line = format!("rename: {e}"),
                                }
                            }
                            continue;
                        }
                    }
                }
                // Settings modal is exclusive: digits, `q`, `n` go to its text input.
                let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c');
                if app_tab == AppTab::Settings && settings.modal.is_some() && !ctrl_c {
                    settings.handle_key(&mut cfg, key);
                    continue;
                }
                if spawn_overlay.is_none()
                    && rename_overlay.is_none()
                    && !show_help
                    && key.modifiers.is_empty()
                    && key.code == KeyCode::Char('n')
                {
                    spawn_overlay = Some(ModalState::new(&cfg));
                    continue;
                }
                if let KeyCode::Char(c) = key.code {
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                        if let Some(tab) = AppTab::from_digit(c) {
                            app_tab = tab;
                            continue;
                        }
                    }
                }
                if ctrl_c {
                    should_quit = true;
                    continue;
                }
                if key.code == KeyCode::Char('q')
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                {
                    should_quit = true;
                    continue;
                }

                if show_help {
                    if key.code == KeyCode::Char('?') || key.code == KeyCode::Esc {
                        show_help = false;
                    }
                    continue;
                }

                if app_tab == AppTab::Settings {
                    // Unhandled keys must not fall through to Fleet actions (x kill, …).
                    settings.handle_key(&mut cfg, key);
                    continue;
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
                        cfg.sessions.group_by,
                        waiting_only,
                    ) {
                        continue;
                    }
                } else if app_tab != AppTab::Fleet {
                    continue;
                }

                if focus == FocusArea::PreviewScroll {
                    if key.code == KeyCode::Esc {
                        focus = FocusArea::Tree;
                        continue;
                    }
                    match key.code {
                        KeyCode::Char(']') | KeyCode::PageDown | KeyCode::Char('j') => {
                            preview_scroll += 3;
                            continue;
                        }
                        KeyCode::Char('[') | KeyCode::PageUp | KeyCode::Char('k') => {
                            preview_scroll = preview_scroll.saturating_sub(3);
                            continue;
                        }
                        _ => {}
                    }
                }

                if side_tab == SideTab::Preview
                    && focus == FocusArea::Tree
                    && matches!(
                        key.code,
                        KeyCode::Char(']') | KeyCode::PageDown | KeyCode::Char('[') | KeyCode::PageUp
                    )
                {
                    match key.code {
                        KeyCode::Char(']') | KeyCode::PageDown => preview_scroll += 3,
                        KeyCode::Char('[') | KeyCode::PageUp => {
                            preview_scroll = preview_scroll.saturating_sub(3);
                        }
                        _ => {}
                    }
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
                                    last_preview_refresh = Instant::now();
                                    tree_dirty = true;
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
                            if std::env::var_os("TMUX").is_some() {
                                match switch_client_to_session(&s.name) {
                                    Ok(()) => {
                                        status_line = format!(
                                            "attached {} — M-u or agent exit reopens cdamux",
                                            s.name
                                        );
                                        tree_dirty = true;
                                    }
                                    Err(e) => status_line = format!("attach: {e}"),
                                }
                            } else {
                                attach_name = Some(s.name.clone());
                            }
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
                    DashboardAction::Spawn => {
                        spawn_overlay = Some(ModalState::new(&cfg));
                    }
                    DashboardAction::KillSession => {
                        if let Some(s) = session_at_row(&rows, selected_row) {
                            match kill_tmux_session(&s.name) {
                                Ok(()) => {
                                    tree_dirty = true;
                                    status_line = format!("killed {}", s.name);
                                    selected_row = selected_row.saturating_sub(1);
                                }
                                Err(e) => status_line = format!("kill: {e}"),
                            }
                        } else {
                            status_line = "select a session to kill (x)".into();
                        }
                    }
                    DashboardAction::ForkSession => {
                        if let Some(s) = session_at_row(&rows, selected_row) {
                            match fork_selected_session(&cfg, s) {
                                Ok(msg) => {
                                    tree_dirty = true;
                                    status_line = msg;
                                }
                                Err(e) => status_line = e,
                            }
                        } else {
                            status_line = "select a session to fork (f)".into();
                        }
                    }
                    DashboardAction::RenameSession => {
                        if let Some(s) = session_at_row(&rows, selected_row) {
                            rename_overlay = Some(RenameState {
                                old_name: s.name.clone(),
                                input: s.name.clone(),
                            });
                        } else {
                            status_line = "select a session to rename (m)".into();
                        }
                    }
                    DashboardAction::ToggleWaitingFilter => {
                        waiting_only = !waiting_only;
                        selected_row = 0;
                        status_line = if waiting_only {
                            "filter: waiting for input (◆)".into()
                        } else {
                            "filter: all sessions".into()
                        };
                    }
                    DashboardAction::FoldAll => {
                        toggle_fold_all(&tree, &mut expanded, group_by);
                    }
                    DashboardAction::CycleGroupBy => {
                        cfg.sessions.group_by = cycle_group_by(cfg.sessions.group_by);
                        expanded = default_expanded(
                            &build_project_session_tree(
                                &cfg.projects,
                                cfg.daemon.idle_seconds_for_needs_input,
                                cfg.sessions.group_by,
                            ),
                            cfg.sessions.group_by,
                        );
                        selected_row = 0;
                        settings.dirty = true;
                        status_line = format!("group_by: {:?}", cfg.sessions.group_by);
                    }
                    DashboardAction::ToggleHelp => show_help = !show_help,
                    DashboardAction::None => {}
                }
                }
                _ => {}
            }
        }
    }

    drop(terminal);
    drop(_guard);

    if let Some(name) = attach_name {
        return crate::tmux_attach_session_with_mode(&name, Some(cfg.sessions.attach_mode));
    }
    if let Some(name) = review_session {
        return crate::run_review_for_session(&name);
    }
    ExitCode::SUCCESS
}

fn session_state_color(state: SessionState) -> Color {
    match state {
        SessionState::Idle => CLR_IDLE,
        SessionState::Active => CLR_OK,
        SessionState::NeedsInput => CLR_WARN,
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

fn row_index_from_mouse(
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
    group_by: GroupBy,
    waiting_only: bool,
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
                *fleet_row = focus_session_in_tree(
                    tree,
                    expanded,
                    &s.name,
                    group_by,
                    waiting_only,
                );
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

fn draw_help_overlay(f: &mut Frame, area: ratatui::layout::Rect) {
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

fn draw_fleet_body(
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

fn draw_tree_column(f: &mut Frame, area: ratatui::layout::Rect, rows: &[TreeRow], selected_row: usize) {
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

fn tree_row_item(row: &TreeRow) -> ListItem<'_> {
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
                Span::styled(format!("{} ", chevron), Style::default().fg(CLR_MUTED)),
                Span::styled("◇ ", Style::default().fg(CLR_ACCENT)),
                Span::styled(
                    format!("{} ", name),
                    Style::default()
                        .fg(CLR_PROJECT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({}) ", session_count),
                    Style::default().fg(CLR_MUTED),
                ),
                Span::styled(path.clone(), Style::default().fg(CLR_MUTED)),
            ]))
        }
        TreeRow::Session(s) | TreeRow::Unassigned(s) => {
            let icon = state_icon(s.state);
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(session_state_color(s.state)),
                ),
                Span::styled(
                    format!("{}  ", s.name),
                    Style::default().fg(CLR_TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}  ", s.agent),
                    Style::default().fg(CLR_MUTED),
                ),
                Span::styled(s.path.clone(), Style::default().fg(CLR_MUTED)),
            ]))
        }
        TreeRow::UnassignedHeader => ListItem::new(Line::from(Span::styled(
            "— unassigned sessions",
            Style::default()
                .fg(CLR_WARN)
                .add_modifier(Modifier::ITALIC),
        ))),
    }
}

fn draw_sidecar(
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

    let preview_idx = if show_chrome { 2 } else { 1 };
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
        assert_eq!(SideTab::Review.next(), SideTab::OrchGraph);
        assert_eq!(SideTab::OrchGraph.next(), SideTab::Preview);
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
                    agent: "—".into(),
                }],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = HashSet::new();
        let collapsed = flatten_tree(&tree, &exp, GroupBy::Project, false);
        assert_eq!(collapsed.len(), 1);
        let exp2: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let open = flatten_tree(&tree, &exp2, GroupBy::Project, false);
        assert_eq!(open.len(), 2);
    }

    #[test]
    fn waiting_filter_hides_idle_sessions() {
        let tree = ProjectSessionTree {
            nodes: vec![cdamux_core::ProjectTreeNode {
                project: Project {
                    name: "p".into(),
                    path: PathBuf::from("/tmp/p"),
                    root_label: "dev".into(),
                },
                sessions: vec![
                    cdamux_core::EnrichedSession {
                        name: "idle".into(),
                        path: "/tmp/p".into(),
                        group_key: "p".into(),
                        state: SessionState::Idle,
                        agent: "claude".into(),
                    },
                    cdamux_core::EnrichedSession {
                        name: "wait".into(),
                        path: "/tmp/p".into(),
                        group_key: "p".into(),
                        state: SessionState::NeedsInput,
                        agent: "cursor".into(),
                    },
                ],
            }],
            unassigned: vec![],
        };
        let exp: HashSet<String> = [project_key(&tree.nodes[0].project.path)]
            .into_iter()
            .collect();
        let rows = flatten_tree(&tree, &exp, GroupBy::Project, true);
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[1], TreeRow::Session(s) if s.name == "wait"));
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

    #[test]
    fn map_waiting_and_help_keys() {
        assert_eq!(
            map_key_event(key(KeyCode::Char('w'))),
            DashboardAction::ToggleWaitingFilter
        );
        assert_eq!(map_key_event(key(KeyCode::Char('F'))), DashboardAction::FoldAll);
        assert_eq!(map_key_event(key(KeyCode::Char('g'))), DashboardAction::CycleGroupBy);
        assert_eq!(map_key_event(key(KeyCode::Char('?'))), DashboardAction::ToggleHelp);
    }
}
