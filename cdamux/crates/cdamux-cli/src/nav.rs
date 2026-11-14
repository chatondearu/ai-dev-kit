//! `cdamux nav` (#337): Fleet tree in a dedicated `cdamux-nav` session.
//!
//! Opening the navigator never splits or join-panes into agent sessions; Enter and
//! `q` use `switch-client` only. Alt-N (example bind) toggles via `toggle_nav_client`.

use crate::fleet_draw::{draw_help_overlay, draw_tree_column, row_index_from_mouse, tree_list_inner};
use crate::fleet_tree::{
    default_expanded, flatten_tree, focus_session_in_tree, move_selection, reanchor_selection,
    session_at_row, set_row_fold, toggle_row_fold, dormant_at_row, TreeRow,
};
use crate::keymap::{fit_footer, map_key_event, DashboardAction};
use crate::refresh::{Interval, StatusCache};
use crate::tui::TerminalGuard;
use cdamux_core::{
    build_project_session_tree_ex, default_client, ensure_nav_session, kill_tmux_session,
    leave_nav_to_session, load_config, nav_return_or_home, toggle_nav_client, NAV_SESSION_NAME,
};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::Paragraph,
    Frame,
};
use std::collections::HashSet;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// Primary binds per PRD-334 (`d` reserved for dismiss in #336).
const FOOTER: &str = "j/k · ⏎ · d · x · ? · q";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivateKind {
    Attach,
    DormantHint,
    ToggleFold,
}

fn activate_kind(rows: &[TreeRow], idx: usize) -> ActivateKind {
    if session_at_row(rows, idx).is_some() {
        ActivateKind::Attach
    } else if dormant_at_row(rows, idx).is_some() {
        ActivateKind::DormantHint
    } else {
        ActivateKind::ToggleFold
    }
}

fn nav_layout(area: Rect) -> (Rect, Rect) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);
    (parts[0], parts[1])
}

fn tree_row_at(
    term: Rect,
    column: u16,
    row: u16,
    row_count: usize,
    selected_row: usize,
) -> Option<usize> {
    let (tree_area, _) = nav_layout(term);
    let inner = tree_list_inner(tree_area, false);
    row_index_from_mouse(inner, column, row, row_count, selected_row)
}

fn draw_nav(
    f: &mut Frame,
    rows: &[TreeRow],
    selected_row: usize,
    hovered_row: Option<usize>,
    status_line: &str,
    show_help: bool,
) {
    let (tree_area, footer_area) = nav_layout(f.area());
    draw_tree_column(f, tree_area, rows, selected_row, hovered_row, false);
    f.render_widget(
        Paragraph::new(vec![
            Line::raw(fit_footer(FOOTER, footer_area.width as usize)),
            Line::raw(status_line.to_string()),
        ]),
        footer_area,
    );
    if show_help {
        draw_help_overlay(f, f.area());
    }
}

fn run_activate(
    rows: &[TreeRow],
    idx: usize,
    expanded: &mut HashSet<String>,
    status_line: &mut String,
) {
    match activate_kind(rows, idx) {
        ActivateKind::Attach => {
            if let Some(s) = session_at_row(rows, idx) {
                match leave_nav_to_session(&s.name) {
                    Ok(()) => *status_line = format!("switched to {}", s.name),
                    Err(e) => *status_line = format!("switch: {e}"),
                }
            }
        }
        ActivateKind::DormantHint => {
            *status_line = "dormant: restore it from Fleet (Alt-P)".into();
        }
        ActivateKind::ToggleFold => {
            toggle_row_fold(rows, idx, expanded);
        }
    }
}

/// Toggle / open the navigator for `client`, or run the in-session Fleet loop.
pub(crate) fn cmd_nav(client: Option<String>, in_session: bool) -> ExitCode {
    if std::env::var_os("TMUX").is_none() {
        eprintln!("cdamux nav: must run inside tmux");
        return ExitCode::from(1);
    }
    if in_session {
        return run_nav();
    }
    let client = match client.map_or_else(default_client, Ok) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cdamux nav: {e}");
            return ExitCode::from(1);
        }
    };
    match toggle_nav_client(&client) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cdamux nav: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_nav() -> ExitCode {
    if let Err(e) = ensure_nav_session() {
        eprintln!("cdamux nav --in-session: {e}");
        return ExitCode::from(1);
    }
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let group_by = cfg.sessions.group_by;
    let build = |keep: &[&std::path::Path]| {
        build_project_session_tree_ex(
            &cfg.projects,
            cfg.daemon.idle_seconds_for_needs_input,
            cfg.daemon.working_warn_seconds,
            group_by,
            keep,
        )
    };
    let mut tree = build(&[]);
    let mut expanded = default_expanded(&tree, group_by);
    let mut status = StatusCache::new(Duration::from_millis(500));
    let mut selected_row = focus_session_in_tree(&tree, &mut expanded, "", group_by, false, &status);

    let (mut terminal, _guard) = match TerminalGuard::enter() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("nav: terminal setup failed: {e}");
            return ExitCode::from(1);
        }
    };

    let mut status_line = format!("navigator · {NAV_SESSION_NAME}");
    let mut hovered_row: Option<usize> = None;
    let mut show_help = false;
    let mut tree_iv = Interval::new(Duration::from_millis(2000));
    let mut tree_dirty = true;
    let mut needs_draw = true;

    loop {
        let now = Instant::now();
        let repos = tree.nodes.iter().map(|n| n.project.path.as_path());
        needs_draw |= status.refresh(repos, now);
        let repos = tree.nodes.iter().map(|n| n.project.path.as_path());
        needs_draw |= status.refresh_runs(repos, now);
        let names = tree
            .nodes
            .iter()
            .flat_map(|n| &n.sessions)
            .chain(&tree.unassigned)
            .map(|s| s.name.as_str());
        needs_draw |= status.refresh_subagents(names, now);
        tree_dirty |= status.refresh_registry(now);

        let mut anchor: Option<String> = None;
        if tree_dirty || tree_iv.due(now) {
            let old = flatten_tree(&tree, &expanded, group_by, false, &status);
            anchor = session_at_row(&old, selected_row).map(|s| s.name.clone());
            let keep: Vec<&std::path::Path> = status.records().iter().map(|r| r.cwd.as_path()).collect();
            tree = build(&keep);
            tree_iv.mark(now);
            tree_dirty = false;
            needs_draw = true;
        }
        let rows = flatten_tree(&tree, &expanded, group_by, false, &status);
        selected_row = reanchor_selection(&rows, selected_row, anchor.as_deref());
        if hovered_row.is_some_and(|h| h >= rows.len()) {
            hovered_row = None;
        }

        if needs_draw {
            needs_draw = false;
            crate::link_ui::draw(&mut terminal, |f| {
                draw_nav(f, &rows, selected_row, hovered_row, &status_line, show_help)
            })
            .ok();
        }

        if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
            continue;
        }
        needs_draw = true;
        match event::read() {
            Ok(Event::Mouse(m)) => {
                let Ok(size) = terminal.size() else { continue };
                let term = Rect {
                    x: 0,
                    y: 0,
                    width: size.width,
                    height: size.height,
                };
                match m.kind {
                    MouseEventKind::Moved => {
                        let next = tree_row_at(term, m.column, m.row, rows.len(), selected_row);
                        if next != hovered_row {
                            hovered_row = next;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        let Some(idx) = tree_row_at(term, m.column, m.row, rows.len(), selected_row)
                        else {
                            continue;
                        };
                        selected_row = idx;
                        run_activate(&rows, selected_row, &mut expanded, &mut status_line);
                    }
                    _ => {}
                }
            }
            Ok(Event::Key(key)) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                if show_help {
                    if key.code == KeyCode::Char('?') || key.code == KeyCode::Esc {
                        show_help = false;
                    }
                    continue;
                }
                match map_key_event(key) {
                    DashboardAction::Quit => {
                        // Hide: switch-client away; keep this process so Alt-N finds a live nav.
                        if let Err(e) = leave_nav_to_session(&nav_return_or_home()) {
                            status_line = format!("quit: {e}");
                        }
                    }
                    DashboardAction::ToggleHelp => show_help = true,
                    action @ (DashboardAction::SelectNext | DashboardAction::SelectPrev) => {
                        let delta = if action == DashboardAction::SelectNext { 1 } else { -1 };
                        selected_row = move_selection(&rows, selected_row, delta);
                    }
                    DashboardAction::Collapse => set_row_fold(&rows, selected_row, &mut expanded, false),
                    DashboardAction::Expand => set_row_fold(&rows, selected_row, &mut expanded, true),
                    DashboardAction::Attach => {
                        run_activate(&rows, selected_row, &mut expanded, &mut status_line);
                    }
                    DashboardAction::KillSession => {
                        if let Some(s) = session_at_row(&rows, selected_row) {
                            match kill_tmux_session(&s.name) {
                                Ok(()) => {
                                    status_line = format!("killed {}", s.name);
                                    tree_dirty = true;
                                }
                                Err(e) => status_line = format!("kill: {e}"),
                            }
                        }
                    }
                    _ if key.code == KeyCode::Char('d') => {
                        status_line = "dismiss: use Fleet after #336".into();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_render::{assert_snapshot, fixture_rows, render};

    #[test]
    fn nav_footer_lists_prd_binds() {
        for part in ["j/k", "⏎", "d", "x", "?", "q"] {
            assert!(FOOTER.contains(part), "{FOOTER}");
        }
    }

    #[test]
    fn narrow_layout_keeps_tree_and_footer() {
        let lines = render(40, 16, |f| draw_nav(f, &fixture_rows(), 2, None, "ok", false));
        assert_snapshot("nav-40x16", &lines);
    }

    #[test]
    fn activate_kind_matches_sidebar() {
        let rows = fixture_rows();
        assert_eq!(activate_kind(&rows, 0), ActivateKind::ToggleFold);
        assert_eq!(activate_kind(&rows, 1), ActivateKind::Attach);
    }
}
