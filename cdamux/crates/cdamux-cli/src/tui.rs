//! Full-screen ratatui dashboard (default `cdamux` with no subcommand).

use crate::app_tabs::{flat_sessions, tab_at_column, AppTab, cycle_group_by};
use crate::fleet_draw::{
    draw_app_screen, draw_rename_modal, review_sidecar_hint, row_index_from_mouse,
    scroll_preview, DrawState, FocusArea, RenameState, SideTab,
};
use crate::foam_tab::{foam_click_line, FoamState};
use crate::graph_view::{load_orch_graph_view, node_session, worker_steps_text};
use crate::fleet_tree::{
    clamp_selection, default_expanded, dormant_at_row, fleet_attention_hint, flatten_tree,
    focus_session_in_tree,
    graph_target_at_row, issue_at_row, move_selection, orch_repo_at_row, project_runs_text,
    reanchor_selection, session_at_row, toggle_fold_all, set_row_fold, toggle_row_fold, GraphTarget,
    TreeRow,
};
use crate::keymap::{map_key_event, DashboardAction};
use crate::refresh::{sidecar_step, Interval, Sidecar, StatusCache};
use crate::settings_form::SettingsForm;
use crate::widgets::{edit_text, wheel_step};
use crate::spawn_modal::{
    hinted_agent_index, draw_modal, draw_overlay_scrim, on_key as spawn_modal_on_key, ModalState, SpawnHint,
    SpawnModalAction,
};
use cdamux_core::{
    build_project_session_tree_ex, capture_pane_text, config_path, current_tmux_session,
    discover_projects, finalize_session_host, kill_tmux_session, load_config, preferred_spawn_agent,
    rename_tmux_session,
    configured_agent_ids, resolve_agent_command, reply_to_ask, restore_one, restore_targets,
    send_to_session,
    session_name_for_path, spawn_tmux_session_with_layout, switch_client_to_session, tail_lines,
    transcript_tail, unique_session_name, ChildKind, ChildRow, Config, EnrichedSession, GroupBy,
    PaneLayout, PreviewSource, ProjectSessionTree, session_registry, SessionChrome,
};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEventKind,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::HashSet;
use std::io::{self, stdout, Stdout};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

enum RenameAction {
    Continue,
    Cancel,
    Confirm,
}

/// Foam walk/render only while the Foam tab is visible (#294 / PRD-293).
///
/// Deep-link from preview text runs on Foam focus and while Foam stays selected —
/// not on every Preview session focus (no auto-switch to Foam from Preview).
fn maybe_refresh_foam_sidecar(
    side_tab: SideTab,
    cwd: &str,
    session_key: &str,
    preview_text: &str,
    foam_for_session: &mut Option<String>,
    foam: &mut FoamState,
    foam_deep_link_applied: &mut Option<String>,
) -> bool {
    if side_tab != SideTab::Foam {
        return false;
    }
    let mut changed = false;
    if foam_for_session.as_deref() != Some(session_key) {
        *foam = FoamState::load(cwd);
        *foam_for_session = Some(session_key.to_string());
        *foam_deep_link_applied = None;
        changed = true;
    }
    if foam.apply_deep_link_from_preview(preview_text, foam_deep_link_applied) {
        changed = true;
    }
    changed
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
    let ids = configured_agent_ids(&cfg.agents);
    let hint = SpawnHint { agent: Some(s.agent.clone()), ..Default::default() };
    let agent = match hinted_agent_index(&hint, &ids) {
        Some(i) => ids[i].clone(),
        None => preferred_spawn_agent(&cfg.agents),
    };
    let cmd = resolve_agent_command(&cfg.agents, &agent).map_err(|e| e.to_string())?;
    match spawn_tmux_session_with_layout(
        &base,
        &path,
        &cmd,
        PaneLayout::Single,
        false,
        Some(agent.as_str()),
        Some(s.name.as_str()),
    ) {
        Ok(o) => Ok(format!("forked '{}' with {}{}", base, agent, o.note())),
        Err(e) => Err(e.to_string()),
    }
}

/// Spawn modal context from the Fleet selection (other tabs: none).
fn spawn_hint(app_tab: AppTab, tree: &ProjectSessionTree, rows: &[TreeRow], idx: usize) -> SpawnHint {
    if app_tab != AppTab::Fleet {
        return SpawnHint::default();
    }
    let selected = session_at_row(rows, idx);
    let repo = orch_repo_at_row(rows, idx);
    let node = tree.nodes.iter().find(|n| {
        selected.is_some_and(|s| n.sessions.iter().any(|x| x.name == s.name))
            || repo.as_deref() == Some(n.project.path.to_string_lossy().as_ref())
    });
    let sessions = node.map_or(&[][..], |n| n.sessions.as_slice());
    SpawnHint {
        session: selected.or(sessions.first()).map(|s| s.name.clone()),
        agent: selected
            .map(|s| s.agent.clone())
            .or_else(|| dormant_at_row(rows, idx).map(|r| r.agent.clone())),
        project_agents: sessions.iter().map(|s| s.agent.clone()).collect(),
    }
}

/// Inside a tmux popup (`display-popup` sets `TMUX` but no `TMUX_PANE`).
fn in_tmux_popup() -> bool {
    std::env::var_os("TMUX").is_some() && std::env::var_os("TMUX_PANE").is_none()
}

/// Inside tmux: switch the client and return a status line. Outside: `None`,
/// the caller exits the dashboard and attaches.
fn switch_or_attach(name: &str) -> Option<Result<String, String>> {
    std::env::var_os("TMUX")?;
    Some(
        switch_client_to_session(name)
            .map(|()| format!("attached {name} — M-u or agent exit reopens cdamux"))
            .map_err(|e| format!("attach: {e}")),
    )
}

fn fetch_preview(session_name: &str, max_lines: u32) -> String {
    let lines = max_lines.clamp(10, 500) as i32;
    match capture_pane_text(session_name, lines, true) {
        Ok(t) if t.trim().is_empty() => "(empty pane)".to_string(),
        Ok(t) => t,
        Err(_) => "(preview unavailable)".to_string(),
    }
}

/// Preview text for a child row: its pane, or its file tail (sub-agent transcripts are
/// formatted, logs raw); `None` when the row has no source.
fn preview_source_text(row: &ChildRow, max_lines: u32) -> Option<String> {
    let n = max_lines.clamp(10, 500) as usize;
    match &row.preview {
        PreviewSource::Pane(name) => Some(fetch_preview(name, max_lines)),
        PreviewSource::FileTail(path) if row.kind == ChildKind::Subagent => {
            Some(transcript_tail(path, n))
        }
        PreviewSource::FileTail(path) => Some(match tail_lines(path, n) {
            Ok(t) if t.trim().is_empty() => format!("(empty file {})", path.display()),
            Ok(t) => t,
            Err(e) => format!("(cannot read {}: {e})", path.display()),
        }),
        PreviewSource::None => None,
    }
}

pub(crate) struct TerminalGuard;

impl TerminalGuard {
    pub(crate) fn enter() -> io::Result<(Terminal<CrosstermBackend<Stdout>>, Self)> {
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
    let tree = build_project_session_tree_ex(
        &cfg.projects,
        cfg.daemon.idle_seconds_for_needs_input,
        cfg.daemon.working_warn_seconds,
        group_by,
        &[],
    );
    let mut expanded = default_expanded(&tree, group_by);
    let mut waiting_only = false;
    let mut show_help = false;
    let mut status = StatusCache::new(Duration::from_millis(500));
    // Opened over a session (popup): start on it, its project unfolded.
    let mut selected_row = current_tmux_session()
        .map(|name| focus_session_in_tree(&tree, &mut expanded, &name, group_by, false, &status))
        .unwrap_or(0);
    let mut side_tab = SideTab::Preview;
    let mut focus = FocusArea::Tree;
    let mut composer_line = String::new();
    let mut preview_text = String::new();
    let mut preview_for_session: Option<String> = None;
    // Only gathered for a session row: tmux is never queried with an empty target.
    let mut session_chrome: Option<SessionChrome> = None;
    let mut foam = FoamState::message("");
    let mut foam_for_session: Option<String> = None;
    // Last foam path focused from preview capture (#269); reset on session change.
    let mut foam_deep_link_applied: Option<String> = None;
    let mut orch_graph = load_orch_graph_view(&GraphTarget::None, 0, 72);
    let mut orch_graph_for_session: Option<String> = None;
    // Graph view `Tab`: which of the selected session's runs; reset when the target changes.
    let mut run_pick = 0usize;
    let mut graph_target_key = String::new();
    // Full-screen Graph view (`g`): selected node index in navigation order.
    let mut graph_view: Option<usize> = None;
    let mut status_line = String::new();
    // Dormant row `x` was pressed on once: a second `x` on it forgets the record.
    let mut pending_forget: Option<String> = None;

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
    // Per-tab data refreshed with the tree, never on every 100 ms poll.
    let mut cached_tab = app_tab;
    let mut discovered = Vec::new();
    let mut flat = Vec::new();
    let mut agent_lines = Vec::new();
    let mut tree_dirty = true;
    // One interval per source; the 100 ms poll only handles input.
    let mut tree_iv = Interval::new(Duration::from_millis(2000));
    let mut preview_iv = Interval::new(Duration::from_millis(800));
    let mut graph_iv = Interval::new(Duration::from_millis(800));
    // Redraw on input or when a refresh changed something visible.
    let mut needs_draw = true;

    while !should_quit && attach_name.is_none() && review_session.is_none() {
        let now = Instant::now();
        let group_by = cfg.sessions.group_by;
        let mut anchor: Option<String> = None;
        let repos = cached_tree.nodes.iter().map(|n| n.project.path.as_path());
        needs_draw |= status.refresh(repos, now);
        let repos = cached_tree.nodes.iter().map(|n| n.project.path.as_path());
        needs_draw |= status.refresh_runs(repos, now);
        let names = cached_tree
            .nodes
            .iter()
            .flat_map(|n| &n.sessions)
            .chain(&cached_tree.unassigned)
            .map(|s| s.name.as_str());
        needs_draw |= status.refresh_subagents(names, now);
        tree_dirty |= status.refresh_registry(now);
        if tree_dirty || app_tab != cached_tab || tree_iv.due(now) {
            // Remember the selected session by name: rows shift when sessions come and go.
            let old_rows = flatten_tree(&cached_tree, &expanded, group_by, waiting_only, &status);
            anchor = session_at_row(&old_rows, selected_row)
                .map(|s| s.name.clone())
                .or_else(|| dormant_at_row(&old_rows, selected_row).map(|r| r.name.clone()));
            let keep: Vec<&std::path::Path> = status.records().iter().map(|r| r.cwd.as_path()).collect();
            cached_tree = build_project_session_tree_ex(
                &cfg.projects,
                cfg.daemon.idle_seconds_for_needs_input,
                cfg.daemon.working_warn_seconds,
                group_by,
                &keep,
            );
            discovered = if app_tab == AppTab::Projects {
                discover_projects(&cfg.projects)
            } else {
                Vec::new()
            };
            flat = if app_tab == AppTab::Sessions {
                flat_sessions(&cfg)
            } else {
                Vec::new()
            };
            agent_lines = if app_tab == AppTab::Agents {
                crate::doctor_agent_lines(&cfg)
            } else {
                Vec::new()
            };
            cached_tab = app_tab;
            tree_iv.mark(now);
            tree_dirty = false;
            needs_draw = true;
        }
        let tree = &cached_tree;
        let rows = flatten_tree(tree, &expanded, group_by, waiting_only, &status);
        selected_row = reanchor_selection(&rows, selected_row, anchor.as_deref());
        if app_tab == AppTab::Sessions && sessions_selected >= flat.len() && !flat.is_empty() {
            sessions_selected = flat.len() - 1;
        }

        let preview_max = cfg.ui.preview_max_lines;
        let session = session_at_row(&rows, selected_row);
        if let Some(s) = session {
            let session_changed = preview_for_session.as_deref() != Some(s.name.as_str());
            match sidecar_step(
                true,
                session_changed,
                side_tab == SideTab::Preview && graph_view.is_none(),
                &mut preview_iv,
                now,
            ) {
                Sidecar::Load => {
                    preview_scroll = 0;
                    preview_text = fetch_preview(&s.name, preview_max);
                    preview_for_session = Some(s.name.clone());
                    session_chrome = Some(SessionChrome::gather(&s.name, &s.path));
                    needs_draw = true;
                }
                Sidecar::Poll => {
                    let text = fetch_preview(&s.name, preview_max);
                    needs_draw |= text != preview_text;
                    preview_text = text;
                    if let Some(c) = session_chrome.as_mut() {
                        let before = c.clone();
                        c.refresh_volatile(&s.name);
                        needs_draw |= *c != before;
                    }
                }
                Sidecar::Idle => {}
            }
            needs_draw |= maybe_refresh_foam_sidecar(
                side_tab,
                &s.path,
                &s.name,
                &preview_text,
                &mut foam_for_session,
                &mut foam,
                &mut foam_deep_link_applied,
            );
        } else if let Some(repo) = orch_repo_at_row(&rows, selected_row) {
            session_chrome = None;
            match rows.get(selected_row) {
                Some(TreeRow::Child { row, project_path, .. }) => {
                    // Session names cannot contain ':', so this key never matches a session.
                    let key = format!("child:{}:{:?}", row.id, row.preview);
                    let changed = preview_for_session.as_deref() != Some(key.as_str());
                    if changed {
                        preview_scroll = 0;
                    }
                    let text = if row.kind == ChildKind::Run {
                        Some(format!(
                            "orch run {}\nstatus {}\n\nOrch graph tab and g show this run.",
                            row.id, row.label
                        ))
                    } else if row.preview == PreviewSource::None && row.kind == ChildKind::Subagent {
                        Some(format!("sub-agent {}\n\n(no transcript yet)", row.label))
                    } else if row.preview == PreviewSource::None {
                        // StatusCache (refresh scheduler) already holds CDAMUX agents: no I/O per tick.
                        let worker = status
                            .workers(std::path::Path::new(project_path))
                            .iter()
                            .find(|w| w.id == row.id);
                        Some(format!(
                            "orch worker #{}\n{}\n\n{}\n\n(no tmux pane — headless orch)",
                            row.id,
                            row.label,
                            worker_steps_text(worker)
                        ))
                    } else {
                        // Pane / file tail: tmux or disk I/O, so only on the preview interval.
                        let visible = side_tab == SideTab::Preview && graph_view.is_none();
                        match sidecar_step(true, changed, visible, &mut preview_iv, now) {
                            Sidecar::Idle => None,
                            Sidecar::Load | Sidecar::Poll => {
                                preview_source_text(row, preview_max)
                            }
                        }
                    };
                    if let Some(text) = text {
                        // Step durations tick: redraw only when the text moved.
                        needs_draw |= text != preview_text;
                        preview_text = text;
                    }
                    preview_for_session = Some(key);
                }
                Some(TreeRow::Project {
                    name,
                    path,
                    worker_count,
                    session_count,
                    ..
                }) => {
                    preview_for_session = None;
                    preview_scroll = 0;
                    preview_text = format!(
                        "project {name}\nsessions: {session_count} · orch workers: {worker_count}\n\nExpand for runs/workers/sessions; Orch graph tab shows the latest run.{}",
                        project_runs_text(&status, path)
                    );
                }
                Some(TreeRow::Dormant { record, .. }) => {
                    preview_for_session = None;
                    preview_scroll = 0;
                    preview_text = format!(
                        "dormant session {}\nagent {} · conversation {}\ncwd {}\n\nEnter restores it · x twice forgets it.",
                        record.name,
                        record.agent,
                        record.agent_session_id.as_deref().unwrap_or("unknown (restores fresh)"),
                        record.cwd.display()
                    );
                }
                _ => {
                    preview_for_session = None;
                    preview_scroll = 0;
                    preview_text = "(select a session for preview)".to_string();
                }
            }
            needs_draw |= maybe_refresh_foam_sidecar(
                side_tab,
                &repo,
                &repo,
                &preview_text,
                &mut foam_for_session,
                &mut foam,
                &mut foam_deep_link_applied,
            );
        } else {
            preview_for_session = None;
            session_chrome = None;
            preview_scroll = 0;
            preview_text = "(select a session for preview)".to_string();
            foam_for_session = None;
            foam_deep_link_applied = None;
            foam = FoamState::message("Select a session to browse project foam.");
        }
        if let Some(area) = sidecar_preview_rect {
            needs_draw |= foam.set_size(area.width, area.height);
        }
        let target = graph_target_at_row(&rows, selected_row, tree, &status);
        let target_key = format!("{target:?}");
        if target_key != graph_target_key {
            run_pick = 0;
            graph_target_key = target_key;
        }
        let graph_key = format!("{graph_target_key}#{run_pick}");
        if orch_graph_for_session.as_deref() != Some(graph_key.as_str())
            || ((side_tab == SideTab::OrchGraph || graph_view.is_some()) && graph_iv.due(now))
        {
            graph_iv.mark(now);
            let view = load_orch_graph_view(&target, run_pick, 72);
            needs_draw |= view.differs(&orch_graph);
            orch_graph = view;
            orch_graph_for_session = Some(graph_key);
        }
        let review_text = review_sidecar_hint(session);

        // Session rows always get a composer; worker ChildRows only when a pending ask
        // needs an `answer` (plan-257).
        let (composer_session, composer_answer, composer_agent_id) =
            composer_target(&rows, selected_row);

        let attention = fleet_attention_hint(&rows);
        let help_status_owned = if app_tab == AppTab::Settings {
            if settings.status.is_empty() {
                status_line.clone()
            } else {
                settings.status.clone()
            }
        } else {
            status_line.clone()
        };
        let help_status = if attention.is_empty() {
            help_status_owned
        } else if help_status_owned.is_empty() {
            attention
        } else {
            format!("{attention}  |  {help_status_owned}")
        };

        let modal_open = spawn_overlay.is_some() || rename_overlay.is_some();
        if needs_draw {
            needs_draw = false;
            terminal
                .draw(|f| {
                    if modal_open {
                        draw_overlay_scrim(f);
                        if let Some(overlay) = &mut spawn_overlay {
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
                                foam_text: &foam.text,
                                review_text: &review_text,
                                orch_graph: &orch_graph,
                                graph_view,
                                composer_line: &composer_line,
                                composer_session,
                                composer_answer,
                                session_chrome: session_chrome.as_ref(),
                                status_line: help_status.as_str(),
                            },
                            &cfg,
                            &settings,
                            &config_path_display,
                            &discovered,
                            &mut projects_scroll,
                            &flat,
                            sessions_selected,
                            &mut sessions_scroll,
                            &agent_lines,
                            waiting_only,
                            show_help,
                            &mut fleet_tree_rect,
                            &mut sidecar_preview_rect,
                        );
                    }
                })
                .ok();
        }

        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            needs_draw = true;
            match event::read() {
                Ok(Event::Mouse(m)) => {
                    // Tab row click → same as digit keys (gaps ignored).
                    if m.kind == MouseEventKind::Down(MouseButton::Left)
                        && spawn_overlay.is_none()
                        && rename_overlay.is_none()
                        && !(app_tab == AppTab::Settings && settings.modal.is_some())
                        && m.row == 0
                    {
                        if let Some(tab) = tab_at_column(m.column) {
                            app_tab = tab;
                        }
                        continue;
                    }
                    let wheel = match m.kind {
                        MouseEventKind::ScrollUp => -3,
                        MouseEventKind::ScrollDown => 3,
                        _ => 0,
                    };
                    // shortcut: wheel anywhere scrolls the modal / tab list (no hit test); add one if a tab gets a second list.
                    if let Some(overlay) = spawn_overlay.as_mut() {
                        overlay.scroll_pick(wheel);
                    } else if app_tab == AppTab::Sessions {
                        sessions_selected = wheel_step(sessions_selected, wheel, flat.len());
                    } else if app_tab == AppTab::Projects {
                        projects_scroll = wheel_step(projects_scroll, wheel, discovered.len());
                    } else if app_tab == AppTab::Fleet && graph_view.is_none() {
                        let foam_line = (side_tab == SideTab::Foam
                            && m.kind == MouseEventKind::Down(MouseButton::Left))
                        .then(|| sidecar_preview_rect.and_then(|a| foam_click_line(a, m.column, m.row)))
                        .flatten();
                        if let Some(line) = foam_line {
                            if foam.click_line(line) {
                                focus = FocusArea::Foam;
                            }
                        } else if focus != FocusArea::Composer
                            && m.kind == MouseEventKind::Down(MouseButton::Left)
                        {
                            if let Some(area) = fleet_tree_rect {
                                if let Some(idx) =
                                    row_index_from_mouse(
                                        area,
                                        m.column,
                                        m.row,
                                        rows.len(),
                                        selected_row,
                                    )
                                {
                                    selected_row = clamp_selection(&rows, idx);
                                    focus = FocusArea::Tree;
                                    preview_for_session = None;
                                    foam_for_session = None;
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
                                    // Wheel up = older lines (scroll counts up from the tail).
                                    preview_scroll = scroll_preview(
                                        preview_scroll,
                                        -wheel,
                                        preview_text.lines().count(),
                                        area.height,
                                    );
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
                        SpawnModalAction::DismissDone { status, session } => {
                            spawn_overlay = None;
                            tree_dirty = true;
                            selected_row = 0;
                            // Land in the new session, like Enter on a Fleet row.
                            match switch_or_attach(&session) {
                                Some(Ok(_)) => {
                                    status_line = status;
                                    // A popup would stay open over it: close it.
                                    should_quit = in_tmux_popup();
                                }
                                Some(Err(e)) => status_line = format!("{status} · {e}"),
                                None => attach_name = Some(session),
                            }
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
                                        let _ = session_registry::rename(&old, &new);
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
                let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c');
                // The composer owns every key (n, q, digits) except Ctrl-C and Esc.
                if focus == FocusArea::Composer && app_tab == AppTab::Fleet {
                    if ctrl_c {
                        should_quit = true;
                        continue;
                    }
                    if key.code == KeyCode::Esc {
                        focus = FocusArea::Tree;
                        continue;
                    }
                    if key.code == KeyCode::Enter {
                        if let Some(name) = composer_session {
                            let text = composer_line.clone();
                            let send_result = if composer_answer {
                                if let Some(id) = composer_agent_id.as_deref() {
                                    reply_to_ask(id, &text)
                                        .map(|_| format!("answered {name}"))
                                        .map_err(|e| e.to_string())
                                } else {
                                    Err("answer: missing agent id".into())
                                }
                            } else {
                                send_to_session(name, &text, true)
                                    .map(|_| format!("sent to {name}"))
                                    .map_err(|e| e.to_string())
                            };
                            match send_result {
                                Ok(msg) => {
                                    status_line = msg;
                                    composer_line.clear();
                                    if !composer_answer {
                                        preview_text = fetch_preview(name, preview_max);
                                        preview_for_session = Some(name.to_string());
                                        preview_iv.mark(Instant::now());
                                    }
                                    tree_dirty = true;
                                }
                                Err(e) => status_line = e,
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

                // Settings modal is exclusive: digits, `q`, `n` go to its text input.
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
                    let hint = spawn_hint(app_tab, tree, &rows, selected_row);
                    spawn_overlay = Some(ModalState::new(&cfg, &hint));
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
                        &tree,
                        &mut expanded,
                        &mut selected_row,
                        &mut app_tab,
                        cfg.sessions.group_by,
                        waiting_only,
                        &status,
                    ) {
                        continue;
                    }
                } else if app_tab != AppTab::Fleet {
                    continue;
                }

                if let Some(sel) = graph_view.as_mut() {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('g') => graph_view = None,
                        // Next run of the same session (the loop reloads on the new key).
                        KeyCode::Tab => {
                            run_pick += 1;
                            *sel = 0;
                        }
                        KeyCode::Down | KeyCode::Right | KeyCode::Char('j') | KeyCode::Char('l') => {
                            *sel = (*sel + 1).min(orch_graph.node_count().saturating_sub(1));
                        }
                        KeyCode::Up | KeyCode::Left | KeyCode::Char('k') | KeyCode::Char('h') => {
                            *sel = sel.saturating_sub(1);
                        }
                        KeyCode::Enter => match orch_graph.node_at(*sel) {
                            Some(node) => match node_session(node) {
                                Some(name) => match switch_or_attach(&name) {
                                    Some(Ok(msg)) => {
                                        status_line = msg;
                                        tree_dirty = true;
                                    }
                                    Some(Err(e)) => status_line = e,
                                    None => attach_name = Some(name),
                                },
                                None => status_line = format!("no session linked to node {}", node.id),
                            },
                            None => status_line = "no graph node selected".into(),
                        },
                        _ => {}
                    }
                    continue;
                }

                if side_tab == SideTab::Preview && focus == FocusArea::Tree {
                    let view = sidecar_preview_rect.map_or(1, |r| r.height);
                    let delta = match key.code {
                        KeyCode::PageUp => view.saturating_sub(1).max(1) as isize,
                        KeyCode::PageDown => -(view.saturating_sub(1).max(1) as isize),
                        KeyCode::Char('[') => 3,
                        KeyCode::Char(']') => -3,
                        _ => 0,
                    };
                    if delta != 0 {
                        preview_scroll =
                            scroll_preview(preview_scroll, delta, preview_text.lines().count(), view);
                        continue;
                    }
                }

                if focus == FocusArea::Foam && !foam.has_tree() {
                    focus = FocusArea::Tree;
                }
                if side_tab == SideTab::Foam && foam.has_tree() {
                    let foam_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    let consumed = if focus == FocusArea::Foam {
                        if key.code == KeyCode::Esc {
                            focus = FocusArea::Tree;
                            true
                        } else {
                            !foam_ctrl && foam.on_key(key.code)
                        }
                    } else if focus == FocusArea::Tree
                        && key.modifiers.is_empty()
                        && key.code == KeyCode::Char('t')
                    {
                        focus = FocusArea::Foam;
                        true
                    } else {
                        false
                    };
                    if consumed {
                        // A consumed key still cancels a pending forget.
                        pending_forget = None;
                        continue;
                    }
                }

                // Any other key cancels a pending forget.
                let forget_armed = pending_forget.take();
                match map_key_event(key) {
                    DashboardAction::Quit => should_quit = true,
                    DashboardAction::CycleSideTab => {
                        side_tab = side_tab.next();
                        if focus == FocusArea::Foam {
                            focus = FocusArea::Tree;
                        }
                    }
                    DashboardAction::FocusComposer => {
                        if side_tab == SideTab::Preview && composer_session.is_some() {
                            focus = FocusArea::Composer;
                        }
                    }
                    DashboardAction::SelectNext => {
                        selected_row = move_selection(&rows, selected_row, 1);
                        preview_for_session = None;
                        foam_for_session = None;
                    }
                    DashboardAction::SelectPrev => {
                        selected_row = move_selection(&rows, selected_row, -1);
                        preview_for_session = None;
                        foam_for_session = None;
                    }
                    DashboardAction::Collapse => {
                        set_row_fold(&rows, selected_row, &mut expanded, false);
                    }
                    DashboardAction::Expand => {
                        set_row_fold(&rows, selected_row, &mut expanded, true);
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
                        Some(TreeRow::Session { session: s, .. }) => {
                            match switch_or_attach(&s.name) {
                                Some(Ok(msg)) => {
                                    status_line = msg;
                                    tree_dirty = true;
                                }
                                Some(Err(e)) => status_line = e,
                                None => attach_name = Some(s.name.clone()),
                            }
                        }
                        Some(TreeRow::Dormant { record, .. }) => {
                            let targets = restore_targets(&record.name)
                                .unwrap_or_else(|| vec![record.clone()]);
                            let mut parts = Vec::new();
                            for rec in &targets {
                                parts.push(match restore_one(rec, &cfg.agents) {
                                    Ok(o) => format!("{}: {}", rec.name, o.label()),
                                    Err(e) => format!("{}: error: {e}", rec.name),
                                });
                            }
                            status_line = parts.join("; ");
                            tree_dirty = true;
                        }
                        _ => toggle_row_fold(&rows, selected_row, &mut expanded),
                        }
                    }
                    DashboardAction::Spawn => {
                        let hint = spawn_hint(app_tab, tree, &rows, selected_row);
                        spawn_overlay = Some(ModalState::new(&cfg, &hint));
                    }
                    DashboardAction::OpenIssue => {
                        match issue_at_row(&rows, selected_row) {
                            Some((cwd, n)) => match open_github_issue(&cwd, n) {
                                Ok(msg) => status_line = msg,
                                Err(e) => status_line = e,
                            },
                            None => {
                                status_line =
                                    "no issue on row (need feat/<n>-* branch or orch worker id)"
                                        .into();
                            }
                        }
                    }
                    DashboardAction::KillSession => {
                        if let Some(r) = dormant_at_row(&rows, selected_row) {
                            if forget_armed.as_deref() == Some(r.name.as_str()) {
                                status_line = match session_registry::remove(&r.name) {
                                    Ok(()) => format!("forgot {}", r.name),
                                    Err(e) => format!("forget {}: {e}", r.name),
                                };
                            } else {
                                pending_forget = Some(r.name.clone());
                                status_line = format!("forget dormant {}? press x again", r.name);
                            }
                        } else if let Some(s) = session_at_row(&rows, selected_row) {
                            match kill_tmux_session(&s.name) {
                                Ok(()) => {
                                    // Closed from cdamux: forget it (an external kill stays restorable).
                                    let _ = session_registry::remove(&s.name);
                                    // Finalize agents/runs for this host so a recreate cannot
                                    // re-attach stuck live graphs (#307). Not used by restart.
                                    let fin = finalize_session_host(&s.name).unwrap_or_default();
                                    tree_dirty = true;
                                    status_line = if fin.agents.is_empty() && fin.runs.is_empty() {
                                        format!("killed {}", s.name)
                                    } else {
                                        format!(
                                            "killed {} (finalized {} agent(s), {} run(s))",
                                            s.name,
                                            fin.agents.len(),
                                            fin.runs.len()
                                        )
                                    };
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
                            &build_project_session_tree_ex(
                                &cfg.projects,
                                cfg.daemon.idle_seconds_for_needs_input,
                                cfg.daemon.working_warn_seconds,
                                cfg.sessions.group_by,
                                &[],
                            ),
                            cfg.sessions.group_by,
                        );
                        selected_row = 0;
                        settings.dirty = true;
                        status_line = format!("group_by: {:?}", cfg.sessions.group_by);
                    }
                    DashboardAction::GraphView => graph_view = Some(0),
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

/// Composer target for the selected row: session name (or worker id), whether Enter
/// should `msg answer`, and the worker agent id when answering.
fn composer_target(
    rows: &[TreeRow],
    idx: usize,
) -> (Option<&str>, bool, Option<&str>) {
    match rows.get(idx) {
        Some(TreeRow::Session { session, .. }) => (
            Some(session.name.as_str()),
            session.pending_ask,
            session.cdamux_id.as_deref(),
        ),
        Some(TreeRow::Child { row, .. })
            if row.kind == ChildKind::Worker && row.pending_ask =>
        {
            (Some(row.id.as_str()), true, row.agent_id.as_deref())
        }
        _ => (None, false, None),
    }
}

fn open_github_issue(cwd: &str, issue: u32) -> Result<String, String> {
    let out = Command::new("gh")
        .args(["issue", "view", &issue.to_string(), "--web"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("gh: {e}"))?;
    if out.status.success() {
        Ok(format!("opened issue #{issue}"))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("gh issue view failed: {}", err.trim()))
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
    tree: &ProjectSessionTree,
    expanded: &mut HashSet<String>,
    fleet_row: &mut usize,
    app_tab: &mut AppTab,
    group_by: GroupBy,
    waiting_only: bool,
    status: &StatusCache,
) -> bool {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if flat.is_empty() {
                return true;
            }
            if *selected + 1 < flat.len() {
                *selected += 1;
            }
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
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
                    status,
                );
                *app_tab = AppTab::Fleet;
            }
            true
        }
        _ => false,
    }
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
        assert_eq!(map_key_event(key(KeyCode::Right)), DashboardAction::Expand);
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
    fn map_waiting_and_help_keys() {
        assert_eq!(
            map_key_event(key(KeyCode::Char('w'))),
            DashboardAction::ToggleWaitingFilter
        );
        assert_eq!(map_key_event(key(KeyCode::Char('F'))), DashboardAction::FoldAll);
        assert_eq!(map_key_event(key(KeyCode::Char('G'))), DashboardAction::CycleGroupBy);
        assert_eq!(map_key_event(key(KeyCode::Char('?'))), DashboardAction::ToggleHelp);
    }

    #[test]
    fn preview_tab_skips_foam_walk() {
        let tmp = tempfile::tempdir().unwrap();
        let foam_dir = tmp.path().join("foam");
        std::fs::create_dir_all(&foam_dir).unwrap();
        std::fs::write(foam_dir.join("index.md"), "# moc").unwrap();
        let cwd = tmp.path().to_str().unwrap();
        let mut for_session = None;
        let mut foam = FoamState::message("(untouched)");
        let mut applied = None;
        assert!(!maybe_refresh_foam_sidecar(
            SideTab::Preview,
            cwd,
            "sess-a",
            "see foam/index.md",
            &mut for_session,
            &mut foam,
            &mut applied,
        ));
        assert_eq!(foam.text, "(untouched)");
        assert!(for_session.is_none());
        assert!(!foam.has_tree());
    }

    #[test]
    fn foam_tab_loads_and_deep_links() {
        let tmp = tempfile::tempdir().unwrap();
        let foam_dir = tmp.path().join("foam");
        std::fs::create_dir_all(&foam_dir).unwrap();
        std::fs::write(foam_dir.join("index.md"), "# moc").unwrap();
        std::fs::write(foam_dir.join("adr.md"), "a").unwrap();
        let cwd = tmp.path().to_str().unwrap();
        let mut for_session = None;
        let mut foam = FoamState::message("");
        let mut applied = None;
        assert!(maybe_refresh_foam_sidecar(
            SideTab::Foam,
            cwd,
            "sess-a",
            "working on foam/adr.md",
            &mut for_session,
            &mut foam,
            &mut applied,
        ));
        assert_eq!(for_session.as_deref(), Some("sess-a"));
        assert_eq!(applied.as_deref(), Some("foam/adr.md"));
        assert_eq!(foam.selected_file(), Some("foam/adr.md"));
        assert!(foam.text.contains("▸     adr.md"), "{}", foam.text);
        assert!(!maybe_refresh_foam_sidecar(
            SideTab::Foam,
            cwd,
            "sess-a",
            "working on foam/adr.md",
            &mut for_session,
            &mut foam,
            &mut applied,
        ));
    }
}
