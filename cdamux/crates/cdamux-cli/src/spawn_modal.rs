//! In-dashboard new session form (PRD-41 phase 3).

use cdamux_core::{
    configured_agent_ids, default_agent_name, default_shell_command, discover_projects,
    list_tmux_sessions, resolve_agent_command, session_name_for_path,
    spawn_tmux_session_with_layout, Config, PaneLayout, SpawnOutcome,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathMode {
    Running,
    Discovered,
    Custom,
}

impl PathMode {
    fn next(self) -> Self {
        match self {
            PathMode::Running => PathMode::Discovered,
            PathMode::Discovered => PathMode::Custom,
            PathMode::Custom => PathMode::Running,
        }
    }

    fn label(self) -> &'static str {
        match self {
            PathMode::Running => "running session cwd",
            PathMode::Discovered => "discovered repo",
            PathMode::Custom => "custom path",
        }
    }
}

pub(crate) struct ModalState {
    path_mode: PathMode,
    running: Vec<(String, PathBuf)>,
    discovered: Vec<PathBuf>,
    pick_index: usize,
    custom_path: String,
    shell_only: bool,
    agent_index: usize,
    agents: Vec<String>,
    layout: PaneLayout,
    editing_custom: bool,
    message: String,
}

impl ModalState {
    pub(crate) fn new(cfg: &Config) -> Self {
        let running: Vec<(String, PathBuf)> = list_tmux_sessions()
            .unwrap_or_default()
            .into_iter()
            .filter(|s| !s.path.is_empty())
            .map(|s| (s.name, PathBuf::from(s.path)))
            .collect();
        let discovered = discover_projects(&cfg.projects)
            .into_iter()
            .map(|p| p.path)
            .collect();
        let agents = configured_agent_ids(&cfg.agents);
        Self {
            path_mode: if running.is_empty() {
                PathMode::Discovered
            } else {
                PathMode::Running
            },
            running,
            discovered,
            pick_index: 0,
            custom_path: String::new(),
            shell_only: false,
            agent_index: 0,
            agents,
            layout: PaneLayout::Single,
            editing_custom: false,
            message: String::new(),
        }
    }

    fn list_len(&self) -> usize {
        match self.path_mode {
            PathMode::Running => self.running.len(),
            PathMode::Discovered => self.discovered.len(),
            PathMode::Custom => 0,
        }
    }

    fn resolved_path(&self) -> Option<PathBuf> {
        match self.path_mode {
            PathMode::Running => self
                .running
                .get(self.pick_index)
                .map(|(_, p)| p.clone()),
            PathMode::Discovered => self.discovered.get(self.pick_index).cloned(),
            PathMode::Custom => {
                let t = self.custom_path.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(t))
                }
            }
        }
    }

    fn cycle_layout(&mut self) {
        self.layout = match self.layout {
            PaneLayout::Single => PaneLayout::Horizontal,
            PaneLayout::Horizontal => PaneLayout::Vertical,
            PaneLayout::Vertical => PaneLayout::Single,
        };
    }

    fn layout_label(&self) -> &'static str {
        match self.layout {
            PaneLayout::Single => "single",
            PaneLayout::Horizontal => "split horizontal",
            PaneLayout::Vertical => "split vertical",
        }
    }
}

pub(crate) enum SpawnModalAction {
    Continue,
    DismissCancel,
    DismissDone { status: String },
}

pub(crate) fn on_key(cfg: &Config, state: &mut ModalState, key: KeyEvent) -> SpawnModalAction {
    if state.editing_custom {
        if key.code == KeyCode::Esc {
            state.editing_custom = false;
            return SpawnModalAction::Continue;
        }
        if key.code == KeyCode::Enter {
            state.editing_custom = false;
            return SpawnModalAction::Continue;
        }
        if key.code == KeyCode::Backspace {
            state.custom_path.pop();
            return SpawnModalAction::Continue;
        }
        if let KeyCode::Char(c) = key.code {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                state.custom_path.push(c);
            }
        }
        return SpawnModalAction::Continue;
    }

    match key.code {
        KeyCode::Esc => SpawnModalAction::DismissCancel,
        KeyCode::Tab => {
            state.path_mode = state.path_mode.next();
            state.pick_index = 0;
            SpawnModalAction::Continue
        }
        KeyCode::Char('s') if key.modifiers.is_empty() => {
            state.shell_only = !state.shell_only;
            SpawnModalAction::Continue
        }
        KeyCode::Char('a') if key.modifiers.is_empty() && !state.shell_only => {
            if !state.agents.is_empty() {
                state.agent_index = (state.agent_index + 1) % state.agents.len();
            }
            SpawnModalAction::Continue
        }
        KeyCode::Char('p') if key.modifiers.is_empty() => {
            state.cycle_layout();
            SpawnModalAction::Continue
        }
        KeyCode::Char('c') if key.modifiers.is_empty() && state.path_mode == PathMode::Custom => {
            state.editing_custom = true;
            SpawnModalAction::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.list_len() > 0 {
                state.pick_index = (state.pick_index + 1) % state.list_len();
            }
            SpawnModalAction::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.list_len() > 0 {
                state.pick_index = (state.pick_index + state.list_len() - 1) % state.list_len();
            }
            SpawnModalAction::Continue
        }
        KeyCode::Enter => {
            if let Some(path) = state.resolved_path() {
                match execute_spawn(cfg, state, path, false) {
                    Ok(msg) => SpawnModalAction::DismissDone { status: msg },
                    Err(e) => {
                        state.message = e;
                        SpawnModalAction::Continue
                    }
                }
            } else {
                state.message = "pick a path".into();
                SpawnModalAction::Continue
            }
        }
        _ => SpawnModalAction::Continue,
    }
}

/// Standalone spawn UI (alternate screen). Prefer overlay via `ModalState` in the dashboard.
pub fn run(cfg: &Config) -> ExitCode {
    use crossterm::event::{self, Event};
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io::{self, stdout, Stdout};

    struct TerminalGuard;

    impl TerminalGuard {
        fn enter() -> io::Result<(Terminal<CrosstermBackend<Stdout>>, Self)> {
            crossterm::terminal::enable_raw_mode()?;
            crossterm::execute!(stdout(), crossterm::terminal::EnterAlternateScreen)?;
            Ok((Terminal::new(CrosstermBackend::new(stdout()))?, Self))
        }
    }

    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            let _ = crossterm::execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }

    let mut state = ModalState::new(cfg);
    let (mut terminal, _guard) = match TerminalGuard::enter() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("spawn modal: {e}");
            return ExitCode::from(1);
        }
    };

    loop {
        terminal.draw(|f| draw_modal(f, &state)).ok();
        if !event::poll(std::time::Duration::from_millis(200)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        match on_key(cfg, &mut state, key) {
            SpawnModalAction::Continue => {}
            SpawnModalAction::DismissCancel => return ExitCode::SUCCESS,
            SpawnModalAction::DismissDone { status } => {
                println!("{status}");
                return ExitCode::SUCCESS;
            }
        }
    }
}

fn execute_spawn(
    cfg: &Config,
    state: &ModalState,
    path: PathBuf,
    switch_to_session: bool,
) -> Result<String, String> {
    let command = if state.shell_only {
        default_shell_command()
    } else {
        let agent = state
            .agents
            .get(state.agent_index)
            .cloned()
            .unwrap_or_else(|| default_agent_name(&cfg.agents));
        resolve_agent_command(&cfg.agents, &agent)
            .map_err(|e| e.to_string())?
    };

    let session_name = session_name_for_path(&path);
    match spawn_tmux_session_with_layout(
        &session_name,
        &path,
        &command,
        state.layout,
        cfg.sessions.attach_mode,
        switch_to_session,
    ) {
        Ok(SpawnOutcome::ReusedExisting) => Ok(format!("reusing session '{}'", session_name)),
        Ok(SpawnOutcome::Created) => Ok(format!(
            "spawned '{}' in {} ({})",
            session_name,
            path.display(),
            state.layout_label()
        )),
        Err(e) => Err(e.to_string()),
    }
}

pub(crate) fn draw_modal(f: &mut Frame, state: &ModalState) {
    let area = centered_rect(70, 70, f.area());
    let block = Block::default()
        .title(" New session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(inner);

    let mode_line = Line::from(vec![
        Span::raw("Path source: "),
        Span::styled(
            state.path_mode.label(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  (Tab cycle)"),
    ]);
    f.render_widget(Paragraph::new(mode_line), chunks[0]);

    let list_body = match state.path_mode {
        PathMode::Running => {
            if state.running.is_empty() {
                "(no sessions with cwd)".to_string()
            } else {
                state
                    .running
                    .iter()
                    .enumerate()
                    .map(|(i, (name, p))| {
                        let mark = if i == state.pick_index { "›" } else { " " };
                        format!("{} {}  {}", mark, name, p.display())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        PathMode::Discovered => {
            if state.discovered.is_empty() {
                "(no discovered repos)".to_string()
            } else {
                state
                    .discovered
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let mark = if i == state.pick_index { "›" } else { " " };
                        format!("{} {}", mark, p.display())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        PathMode::Custom => {
            if state.editing_custom {
                format!("path: {}█", state.custom_path)
            } else {
                format!(
                    "path: {}\n(c) edit custom path",
                    if state.custom_path.is_empty() {
                        "<empty>"
                    } else {
                        state.custom_path.as_str()
                    }
                )
            }
        }
    };
    f.render_widget(Paragraph::new(list_body), chunks[1]);

    let opts = if state.shell_only {
        format!(
            "shell only: yes (bash in primary pane)\nlayout: {}  (p cycle)\nsecondary pane: shell when split",
            state.layout_label()
        )
    } else {
        let a = state
            .agents
            .get(state.agent_index)
            .map(|s| s.as_str())
            .unwrap_or("?");
        format!(
            "agent: {}  (a cycle)  |  shell only: no  (s toggle)\nlayout: {}  (p cycle)\nsecondary pane: shell when split",
            a,
            state.layout_label()
        )
    };
    f.render_widget(Paragraph::new(opts), chunks[2]);

    let help = if state.message.is_empty() {
        "Enter create · Esc cancel · j/k pick · Tab mode · s shell · a agent · p layout"
    } else {
        state.message.as_str()
    };
    f.render_widget(Paragraph::new(help), chunks[3]);
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
