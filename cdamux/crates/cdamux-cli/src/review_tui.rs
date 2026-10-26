//! Ratatui UI to select diff hunks and send a review prompt to an agent session.

use cdamux_core::{DiffHunk, format_review_prompt, send_to_session, SendError};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use std::io::{stdout, Stdout};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    None,
    Quit,
    Next,
    Prev,
    Toggle,
    ToggleAll,
    Send,
}

pub fn map_review_key(key: KeyEvent) -> ReviewAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return ReviewAction::Quit;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => ReviewAction::Quit,
        KeyCode::Down | KeyCode::Char('j') => ReviewAction::Next,
        KeyCode::Up | KeyCode::Char('k') => ReviewAction::Prev,
        KeyCode::Char(' ') => ReviewAction::Toggle,
        KeyCode::Char('a') => ReviewAction::ToggleAll,
        KeyCode::Enter => ReviewAction::Send,
        _ => ReviewAction::None,
    }
}

pub fn hunk_list_label(h: &DiffHunk, selected: bool) -> String {
    let mark = if selected { "[x]" } else { "[ ]" };
    format!("{} {} — {}", mark, h.path, h.header)
}

/// Run the review TUI; returns exit code (0 on successful send).
pub fn run_review_tui(
    session: &str,
    instruction: &str,
    hunks: &[DiffHunk],
) -> ExitCode {
    if hunks.is_empty() {
        eprintln!("review: no diff hunks (working tree clean?)");
        return ExitCode::from(1);
    }

    let mut selected = vec![true; hunks.len()];
    let mut cursor = 0usize;

    let mut stdout = stdout();
    enable_raw_mode().ok();
    stdout.execute(EnterAlternateScreen).ok();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("terminal");

    let result = loop {
        terminal
            .draw(|f| draw_review(f, session, instruction, hunks, &selected, cursor))
            .ok();

        if event::poll(std::time::Duration::from_millis(200)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match map_review_key(key) {
                    ReviewAction::Quit => break Err(()),
                    ReviewAction::Next => {
                        cursor = (cursor + 1).min(hunks.len() - 1);
                    }
                    ReviewAction::Prev => {
                        cursor = cursor.saturating_sub(1);
                    }
                    ReviewAction::Toggle => {
                        selected[cursor] = !selected[cursor];
                    }
                    ReviewAction::ToggleAll => {
                        let any_off = selected.iter().any(|&s| !s);
                        for s in &mut selected {
                            *s = any_off;
                        }
                    }
                    ReviewAction::Send => {
                        let picked: Vec<DiffHunk> = hunks
                            .iter()
                            .zip(selected.iter())
                            .filter(|(_, s)| **s)
                            .map(|(h, _)| h.clone())
                            .collect();
                        if picked.is_empty() {
                            continue;
                        }
                        let prompt = format_review_prompt(instruction, &picked);
                        match send_to_session(session, &prompt, true) {
                            Ok(()) => break Ok(()),
                            Err(SendError::SessionNotFound(name)) => {
                                eprintln!("review: no tmux session named '{name}'");
                                break Err(());
                            }
                            Err(e) => {
                                eprintln!("review: send failed: {e}");
                                break Err(());
                            }
                        }
                    }
                    ReviewAction::None => {}
                }
            }
        }
    };

    restore_terminal(&mut terminal);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::from(1),
    }
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
    disable_raw_mode().ok();
    terminal.backend_mut().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
}

fn draw_review(
    f: &mut ratatui::Frame,
    session: &str,
    instruction: &str,
    hunks: &[DiffHunk],
    selected: &[bool],
    cursor: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(f.area());

    let title = Paragraph::new(Line::from(vec![
        Span::styled("cdamux review", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(" → session {}", session)),
    ]))
    .block(Block::default().borders(Borders::ALL).title("hunks"));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = hunks
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let style = if i == cursor {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(hunk_list_label(h, selected[i])).style(style)
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(instruction.chars().take(48).collect::<String>()),
    );
    f.render_widget(list, chunks[1]);

    let help = Paragraph::new("j/k move · Space toggle · a all · Enter send · q quit")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, chunks[2]);
}
