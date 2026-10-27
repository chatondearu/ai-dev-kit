//! In-dashboard new session form (PRD-41 phase 3).

use cdamux_core::{
    configured_agent_ids, current_branch, default_agent_name, default_shell_command,
    discover_projects, git_toplevel, list_local_branches, list_tmux_sessions,
    preferred_spawn_agent_index, resolve_agent_command, resolve_worktree_dest,
    session_name_for_path, spawn_tmux_session_with_layout, unique_session_name,
    worktree_add_new_branch, Config,
    PaneLayout, SpawnOutcome,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    editing_new_branch: bool,
    branches: Vec<String>,
    branch_index: usize,
    worktree_mode: bool,
    new_branch: String,
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
        let agent_index = preferred_spawn_agent_index(&cfg.agents, &agents);
        let mut state = Self {
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
            agent_index,
            agents,
            layout: PaneLayout::Single,
            editing_custom: false,
            editing_new_branch: false,
            branches: Vec::new(),
            branch_index: 0,
            worktree_mode: false,
            new_branch: String::new(),
            message: String::new(),
        };
        state.sync_git_fields();
        state
    }

    pub(crate) fn sync_git_fields(&mut self) {
        self.branches.clear();
        self.branch_index = 0;
        if let Some(path) = self.resolved_path() {
            let repo = git_toplevel(&path).unwrap_or(path);
            self.branches = list_local_branches(&repo);
            if let Some(head) = current_branch(&repo) {
                if let Some(i) = self.branches.iter().position(|b| b == &head) {
                    self.branch_index = i;
                }
            }
            if self.new_branch.is_empty() && !self.branches.is_empty() {
                let base = self
                    .branches
                    .get(self.branch_index)
                    .map(|s| s.as_str())
                    .unwrap_or("branch");
                self.new_branch = format!("{base}-wt");
            }
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
    if state.editing_new_branch {
        if key.code == KeyCode::Esc {
            state.editing_new_branch = false;
            return SpawnModalAction::Continue;
        }
        if key.code == KeyCode::Enter {
            state.editing_new_branch = false;
            return SpawnModalAction::Continue;
        }
        if key.code == KeyCode::Backspace {
            state.new_branch.pop();
            return SpawnModalAction::Continue;
        }
        if let KeyCode::Char(c) = key.code {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                state.new_branch.push(c);
            }
        }
        return SpawnModalAction::Continue;
    }

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
            state.sync_git_fields();
            SpawnModalAction::Continue
        }
        KeyCode::Char('b') if key.modifiers.is_empty() && !state.branches.is_empty() => {
            state.branch_index = (state.branch_index + 1) % state.branches.len();
            SpawnModalAction::Continue
        }
        KeyCode::Char('w') if key.modifiers.is_empty() => {
            state.worktree_mode = !state.worktree_mode;
            SpawnModalAction::Continue
        }
        KeyCode::Char('g') if key.modifiers.is_empty() && state.worktree_mode => {
            state.editing_new_branch = true;
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
                state.sync_git_fields();
            }
            SpawnModalAction::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.list_len() > 0 {
                state.pick_index = (state.pick_index + state.list_len() - 1) % state.list_len();
                state.sync_git_fields();
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
    let spawn_path = if state.worktree_mode {
        let root = cfg
            .worktrees
            .root
            .as_ref()
            .ok_or_else(|| "worktree mode requires [worktrees].root in config".to_string())?;
        let repo = git_toplevel(&path)
            .ok_or_else(|| "selected path is not inside a git repository".to_string())?;
        let base = state
            .branches
            .get(state.branch_index)
            .map(|s| s.as_str())
            .ok_or_else(|| "pick a base branch (b)".to_string())?;
        let new_branch = state.new_branch.trim();
        if new_branch.is_empty() {
            return Err("set new branch name (g)".into());
        }
        let dest = resolve_worktree_dest(root, new_branch);
        worktree_add_new_branch(&repo, &dest, new_branch, base)
            .map_err(|e| e.to_string())?;
        dest
    } else {
        path
    };

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

    let session_name = unique_session_name(&session_name_for_path(&spawn_path));
    let agent_label: Option<&str> = if state.shell_only {
        Some("shell")
    } else {
        state.agents.get(state.agent_index).map(|s| s.as_str())
    };

    match spawn_tmux_session_with_layout(
        &session_name,
        &spawn_path,
        &command,
        state.layout,
        cfg.sessions.attach_mode,
        switch_to_session,
        agent_label,
    ) {
        Ok(SpawnOutcome::ReusedExisting) => Ok(format!("reusing session '{}'", session_name)),
        Ok(SpawnOutcome::Created) => Ok(format!(
            "spawned '{}' in {} ({})",
            session_name,
            spawn_path.display(),
            state.layout_label()
        )),
        Err(e) => Err(e.to_string()),
    }
}

const OVERLAY_SCRIM: Color = Color::Rgb(10, 12, 16);
const MODAL_BG: Color = Color::Rgb(22, 26, 34);
const MODAL_FG: Color = Color::Rgb(230, 234, 240);

/// Opaque scrim over the dashboard (call before `draw_modal`).
pub(crate) fn draw_overlay_scrim(f: &mut Frame) {
    f.render_widget(
        Block::default().style(Style::default().bg(OVERLAY_SCRIM)),
        f.area(),
    );
}

pub(crate) fn draw_modal(f: &mut Frame, state: &ModalState) {
    let area = centered_rect(70, 70, f.area());
    let block = Block::default()
        .title(" New session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(MODAL_BG).fg(MODAL_FG));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let fill = Block::default().style(Style::default().bg(MODAL_BG));
    f.render_widget(fill, inner);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(3),
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
    f.render_widget(
        Paragraph::new(mode_line).style(Style::default().bg(MODAL_BG).fg(MODAL_FG)),
        chunks[0],
    );

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
    f.render_widget(
        Paragraph::new(list_body).style(Style::default().bg(MODAL_BG).fg(MODAL_FG)),
        chunks[1],
    );

    let git_line = if state.branches.is_empty() {
        "git: (no repo at path)".to_string()
    } else {
        let base = state
            .branches
            .get(state.branch_index)
            .map(|s| s.as_str())
            .unwrap_or("?");
        if state.worktree_mode {
            if state.editing_new_branch {
                format!(
                    "worktree: yes  base: {}  new branch: {}█  (g edit, w off)",
                    base,
                    state.new_branch
                )
            } else {
                format!(
                    "worktree: yes  base: {} (b)  new: {}  (g edit, w off)",
                    base,
                    if state.new_branch.is_empty() {
                        "<empty>"
                    } else {
                        state.new_branch.as_str()
                    }
                )
            }
        } else {
            format!("branch: {} (b)  worktree: no (w)", base)
        }
    };
    f.render_widget(
        Paragraph::new(git_line).style(Style::default().bg(MODAL_BG).fg(MODAL_FG)),
        chunks[2],
    );

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
    f.render_widget(
        Paragraph::new(opts).style(Style::default().bg(MODAL_BG).fg(MODAL_FG)),
        chunks[3],
    );

    let help = if state.message.is_empty() {
        "Enter create · Esc cancel · j/k pick · Tab mode · b branch · w worktree · s shell · a agent · p layout"
    } else {
        state.message.as_str()
    };
    f.render_widget(
        Paragraph::new(help).style(Style::default().bg(MODAL_BG).fg(MODAL_FG)),
        chunks[4],
    );
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
