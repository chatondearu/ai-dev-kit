//! `cdamux sidebar` (#321): the Fleet tree in a narrow pane that follows one tmux client.
//!
//! The launcher splits a pane next to the client's active pane and runs this same
//! binary in it (`--in-pane`). Moving the cursor onto a session switches that client
//! to it and moves the pane along (`cdamux_core::follow_session`); the popup flow is
//! untouched. One sidebar per client, found again through a pane option.

use crate::fleet_draw::{draw_tree_column, row_index_from_mouse, tree_list_inner};
use crate::fleet_tree::{
    default_expanded, flatten_tree, focus_session_in_tree, move_selection, reanchor_selection,
    session_at_row, set_row_fold, toggle_row_fold, dormant_at_row, TreeRow,
};
use crate::keymap::{fit_footer, map_key_event, DashboardAction};
use crate::refresh::{Interval, StatusCache};
use crate::tui::TerminalGuard;
use cdamux_core::{
    build_project_session_tree_ex, client_session, default_client, find_sidebar, focus_main_pane,
    follow_session, load_config, spawn_sidebar, SidebarSide,
};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Paragraph,
    text::Line,
    Frame,
};
use std::collections::HashSet;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// A cursor that rests this long on a session switches to it (rapid moves do not thrash).
const SWITCH_DEBOUNCE: Duration = Duration::from_millis(150);
const FOLLOW_EVERY: Duration = Duration::from_millis(500);
// Keep ≤4 short segments so fit_footer still shows hover/click at 32 cols.
const FOOTER: &str = "j/k · hover · ⏎/click · q";

/// What Enter / left-click do on a tree row (shared so click cannot drift from keyboard).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivateKind {
    /// Live session → `follow_session` + `focus_main_pane`.
    Attach,
    DormantHint,
    /// Project / header / child: fold or select only.
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

fn tree_row_at(
    term: Rect,
    column: u16,
    row: u16,
    row_count: usize,
    selected_row: usize,
) -> Option<usize> {
    let (tree_area, _) = sidebar_layout(term);
    let inner = tree_list_inner(tree_area, false);
    row_index_from_mouse(inner, column, row, row_count, selected_row)
}

/// Enter / left-click: same Attach path for live sessions.
fn run_activate(
    rows: &[TreeRow],
    idx: usize,
    expanded: &mut HashSet<String>,
    client: &str,
    pane: &str,
    side: SidebarSide,
    width: u16,
    pending: &mut Option<(String, Instant)>,
    shown: &mut String,
    status_line: &mut String,
) {
    match activate_kind(rows, idx) {
        ActivateKind::Attach => {
            if let Some(s) = session_at_row(rows, idx) {
                // Land on the session now, whatever the debounce says, then hand over focus.
                let name = s.name.clone();
                *pending = None;
                let moved = follow_session(client, pane, &name, side, width, true);
                match moved.and_then(|()| focus_main_pane(pane)) {
                    Ok(()) => *shown = name,
                    Err(e) => *status_line = format!("focus: {e}"),
                }
            }
        }
        ActivateKind::DormantHint => {
            *status_line = "dormant: restore it from Fleet".into();
        }
        ActivateKind::ToggleFold => {
            toggle_row_fold(rows, idx, expanded);
        }
    }
}

pub(crate) fn cmd_sidebar(
    side: SidebarSide,
    width: u16,
    client: Option<String>,
    in_pane: bool,
) -> ExitCode {
    if std::env::var_os("TMUX").is_none() {
        eprintln!("cdamux sidebar: must run inside tmux");
        return ExitCode::from(1);
    }
    let client = match client.map_or_else(default_client, Ok) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cdamux sidebar: {e}");
            return ExitCode::from(1);
        }
    };
    if in_pane {
        return run_sidebar(&client, side, width);
    }
    match launch(&client, side, width) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cdamux sidebar: {e}");
            ExitCode::from(1)
        }
    }
}

/// Open the sidebar for `client`, or bring its existing one to the client's session.
fn launch(client: &str, side: SidebarSide, width: u16) -> Result<(), String> {
    if let Some(pane) = find_sidebar(client) {
        let session = client_session(client)?;
        return follow_session(client, &pane, &session, side, width, true);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = [
        exe.display().to_string(),
        "sidebar".into(),
        "--in-pane".into(),
        "--client".into(),
        client.into(),
        if side == SidebarSide::Left { "--left" } else { "--right" }.into(),
        "--width".into(),
        width.to_string(),
    ];
    spawn_sidebar(client, side, width, &command).map(drop)
}

fn sidebar_layout(area: Rect) -> (Rect, Rect) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(area);
    (parts[0], parts[1])
}

fn draw_sidebar(
    f: &mut Frame,
    rows: &[TreeRow],
    selected_row: usize,
    hovered_row: Option<usize>,
    status_line: &str,
) {
    let (tree_area, footer_area) = sidebar_layout(f.area());
    draw_tree_column(f, tree_area, rows, selected_row, hovered_row, false);
    f.render_widget(
        Paragraph::new(vec![
            Line::raw(fit_footer(FOOTER, footer_area.width as usize)),
            Line::raw(status_line.to_string()),
        ]),
        footer_area,
    );
}

fn run_sidebar(client: &str, side: SidebarSide, width: u16) -> ExitCode {
    let Ok(pane) = std::env::var("TMUX_PANE") else {
        eprintln!("cdamux sidebar --in-pane: TMUX_PANE is not set");
        return ExitCode::from(1);
    };
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
    // Session the pane sits in; the cursor starts on it.
    let mut shown = client_session(client).unwrap_or_default();
    let mut selected_row = focus_session_in_tree(&tree, &mut expanded, &shown, group_by, false, &status);

    let (mut terminal, _guard) = match TerminalGuard::enter() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sidebar: terminal setup failed: {e}");
            return ExitCode::from(1);
        }
    };
    // Mouse capture stays on (TerminalGuard): hover paint-only; left-click → Attach path.

    let mut status_line = String::new();
    let mut pending: Option<(String, Instant)> = None;
    let mut hovered_row: Option<usize> = None;
    let mut tree_iv = Interval::new(Duration::from_millis(2000));
    let mut follow_iv = Interval::new(FOLLOW_EVERY);
    let mut tree_dirty = true;
    let mut needs_draw = true;
    let mut should_quit = false;

    while !should_quit {
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
        let mut rows = flatten_tree(&tree, &expanded, group_by, false, &status);
        selected_row = reanchor_selection(&rows, selected_row, anchor.as_deref());
        if hovered_row.is_some_and(|h| h >= rows.len()) {
            hovered_row = None;
        }

        if let Some((name, since)) = &pending {
            if now.duration_since(*since) >= SWITCH_DEBOUNCE {
                match follow_session(client, &pane, name, side, width, true) {
                    Ok(()) => shown.clone_from(name),
                    Err(e) => status_line = format!("switch: {e}"),
                }
                pending = None;
                needs_draw = true;
            }
        } else if follow_iv.due(now) {
            follow_iv.mark(now);
            // The client moved on its own (prefix-s, M-u, …): bring the pane along, focus untouched.
            if let Ok(current) = client_session(client) {
                if current != shown {
                    match follow_session(client, &pane, &current, side, width, false) {
                        Ok(()) => {
                            selected_row =
                                focus_session_in_tree(&tree, &mut expanded, &current, group_by, false, &status);
                            rows = flatten_tree(&tree, &expanded, group_by, false, &status);
                            shown = current;
                        }
                        Err(e) => status_line = format!("follow: {e}"),
                    }
                    needs_draw = true;
                }
            }
        }

        if needs_draw {
            needs_draw = false;
            crate::link_ui::draw(&mut terminal, |f| {
                draw_sidebar(f, &rows, selected_row, hovered_row, &status_line)
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
                        // Paint only — never arm the switch debounce from hover.
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
                        run_activate(
                            &rows,
                            selected_row,
                            &mut expanded,
                            client,
                            &pane,
                            side,
                            width,
                            &mut pending,
                            &mut shown,
                            &mut status_line,
                        );
                    }
                    _ => {}
                }
            }
            Ok(Event::Key(key)) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                match map_key_event(key) {
                    DashboardAction::Quit => should_quit = true,
                    action @ (DashboardAction::SelectNext | DashboardAction::SelectPrev) => {
                        let delta = if action == DashboardAction::SelectNext { 1 } else { -1 };
                        selected_row = move_selection(&rows, selected_row, delta);
                        pending = session_at_row(&rows, selected_row)
                            .filter(|s| s.name != shown)
                            .map(|s| (s.name.clone(), Instant::now()));
                    }
                    DashboardAction::Collapse => set_row_fold(&rows, selected_row, &mut expanded, false),
                    DashboardAction::Expand => set_row_fold(&rows, selected_row, &mut expanded, true),
                    DashboardAction::Attach => run_activate(
                        &rows,
                        selected_row,
                        &mut expanded,
                        client,
                        &pane,
                        side,
                        width,
                        &mut pending,
                        &mut shown,
                        &mut status_line,
                    ),
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
    use crate::fleet_draw::{CLR_HOVER_BG, CLR_SEL_BG};
    use crate::test_render::{assert_snapshot, fixture_rows, render};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn narrow_layout_keeps_statuses_and_footer() {
        let lines = render(32, 16, |f| draw_sidebar(f, &fixture_rows(), 2, None, "switch: demo"));
        assert_snapshot("sidebar-32x16", &lines);
    }

    #[test]
    fn hover_style_differs_from_selection() {
        let rows = fixture_rows();
        let selected = 2usize;
        let hovered = 3usize;
        assert_ne!(selected, hovered);
        let mut term = Terminal::new(TestBackend::new(40, 16)).unwrap();
        term.draw(|f| draw_sidebar(f, &rows, selected, Some(hovered), ""))
            .unwrap();
        let buf = term.backend().buffer();
        let (tree_area, _) = sidebar_layout(buf.area);
        let inner = tree_list_inner(tree_area, false);
        // Row y in the list = selected/hovered with offset 0 (both visible near top).
        let sel_cell = &buf[(inner.x, inner.y + selected as u16)];
        let hover_cell = &buf[(inner.x, inner.y + hovered as u16)];
        assert_eq!(sel_cell.bg, CLR_SEL_BG, "selected row uses selection bg");
        assert_eq!(hover_cell.bg, CLR_HOVER_BG, "hovered row uses hover bg");
        assert_ne!(CLR_HOVER_BG, CLR_SEL_BG);
    }

    #[test]
    fn mouse_row_clears_outside_tree() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 16,
        };
        let (tree_area, _) = sidebar_layout(area);
        let inner = tree_list_inner(tree_area, false);
        assert!(row_index_from_mouse(inner, inner.x, inner.y, 5, 0).is_some());
        assert!(row_index_from_mouse(inner, inner.x, area.height - 1, 5, 0).is_none());
    }

    #[test]
    fn click_maps_to_same_activate_as_enter() {
        let rows = fixture_rows();
        // project, live session, worker child, collapsed project, unassigned header, live session
        assert_eq!(activate_kind(&rows, 0), ActivateKind::ToggleFold);
        assert_eq!(activate_kind(&rows, 1), ActivateKind::Attach);
        assert_eq!(activate_kind(&rows, 2), ActivateKind::Attach);
        assert_eq!(activate_kind(&rows, 3), ActivateKind::ToggleFold);
        assert_eq!(activate_kind(&rows, 5), ActivateKind::ToggleFold);
        assert_eq!(activate_kind(&rows, 6), ActivateKind::ToggleFold);
        assert_eq!(activate_kind(&rows, 7), ActivateKind::Attach);

        let dormant = crate::test_render::fixture_dormant_rows();
        let di = dormant
            .iter()
            .position(|r| matches!(r, TreeRow::Dormant { .. }))
            .expect("dormant row");
        assert_eq!(activate_kind(&dormant, di), ActivateKind::DormantHint);
    }

    #[test]
    fn left_click_hit_selects_session_row() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 16,
        };
        let rows = fixture_rows();
        // List row 1 (first live session) with selection at 0 → no scroll offset.
        let (tree_area, _) = sidebar_layout(area);
        let inner = tree_list_inner(tree_area, false);
        let idx = tree_row_at(area, inner.x, inner.y + 1, rows.len(), 0);
        assert_eq!(idx, Some(1));
        assert_eq!(activate_kind(&rows, idx.unwrap()), ActivateKind::Attach);
    }
}
