//! Shared render harness for TUI tests: draw into a `TestBackend`, return text lines,
//! compare against golden snapshots in `tests/snapshots/`.
//!
//! Snapshots capture text only (no colors) and are fully deterministic: fixed
//! fixtures, no clock, no tmux, no filesystem lookups.
//! Rewrite them with `CDAMUX_UPDATE_SNAPSHOTS=1 cargo test`.

use crate::app_tabs::AppTab;
use crate::fleet_draw::{draw_app_screen, DrawState, FocusArea, SideTab};
use crate::fleet_tree::TreeRow;
use crate::graph_view::OrchGraphView;
use crate::settings_form::SettingsForm;
use crate::spawn_modal::{draw_modal, ModalState};
use cdamux_core::{
    ChildKind, ChildRow, Config, EnrichedSession, OrchGraphFile, OrchWorkerStatus, PreviewSource,
    ProjectRoot, SessionChrome, SessionState,
};
use ratatui::{backend::TestBackend, Frame, Terminal};
use std::path::PathBuf;

pub(crate) const SIZES: [(u16, u16); 3] = [(80, 24), (120, 40), (200, 50)];

pub(crate) fn buffer_lines(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// Draw one frame at `w`x`h` and return the screen as text rows.
pub(crate) fn render(w: u16, h: u16, draw: impl FnOnce(&mut Frame)) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(draw).unwrap();
    buffer_lines(&term)
}

/// Compare `lines` with `tests/snapshots/<name>.txt` (rewrite when `CDAMUX_UPDATE_SNAPSHOTS` is set).
pub(crate) fn assert_snapshot(name: &str, lines: &[String]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"));
    // Trailing blanks are noise in diffs.
    let actual: String = lines.iter().map(|l| format!("{}\n", l.trim_end())).collect();
    if std::env::var_os("CDAMUX_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("missing snapshot {}; run CDAMUX_UPDATE_SNAPSHOTS=1 cargo test", path.display())
    });
    assert!(
        expected == actual,
        "snapshot {name} changed ({}); rerun with CDAMUX_UPDATE_SNAPSHOTS=1 to accept\n--- expected\n{expected}--- actual\n{actual}",
        path.display()
    );
}

fn session(name: &str, path: &str, state: SessionState, agent: &str, issue: Option<u32>) -> EnrichedSession {
    EnrichedSession {
        name: name.into(),
        path: path.into(),
        group_key: String::new(),
        state,
        agent: agent.into(),
        issue,
        ..Default::default()
    }
}

fn row(session: EnrichedSession, depth: usize, expanded: Option<bool>) -> TreeRow {
    TreeRow::Session { session, depth, expanded }
}

fn worker(id: &str, phase: &str, message: &str) -> TreeRow {
    TreeRow::Child {
        row: ChildRow::from_orch_worker(
            &OrchWorkerStatus {
                id: id.into(),
                phase: phase.into(),
                message: message.into(),
                ..Default::default()
            },
            false,
        ),
        depth: 1,
        project_path: "/src/alpha".into(),
    }
}

pub(crate) fn fixture_rows() -> Vec<TreeRow> {
    vec![
        TreeRow::Project {
            key: "alpha".into(),
            name: "alpha".into(),
            path: "/src/alpha".into(),
            session_count: 2,
            worker_count: 2,
            expanded: true,
        },
        row(session("alpha-main", "/src/alpha", SessionState::Active, "claude", None), 1, None),
        row(session("alpha-42", "/src/alpha", SessionState::NeedsInput, "cursor", Some(42)), 1, None),
        worker("7", "running", "implementing parser"),
        worker("8", "blocked", "waiting on review"),
        TreeRow::Project {
            key: "beta".into(),
            name: "beta".into(),
            path: "/src/beta".into(),
            session_count: 0,
            worker_count: 0,
            expanded: false,
        },
        TreeRow::UnassignedHeader,
        row(session("scratch", "/tmp/scratch", SessionState::Idle, "shell", None), 1, None),
    ]
}

/// One parent session with a forked child and grandchild, plus a collapsed parent
/// (its hidden child still counts in the project total).
pub(crate) fn fixture_nested_rows() -> Vec<TreeRow> {
    vec![
        TreeRow::Project {
            key: "alpha".into(),
            name: "alpha".into(),
            path: "/src/alpha".into(),
            session_count: 5,
            worker_count: 0,
            expanded: true,
        },
        row(session("alpha-main", "/src/alpha", SessionState::Active, "claude", None), 1, Some(true)),
        row(session("alpha-fork", "/src/alpha", SessionState::NeedsInput, "claude", Some(42)), 2, Some(true)),
        row(session("alpha-fork-2", "/src/alpha", SessionState::Idle, "cursor", None), 3, None),
        row(session("alpha-side", "/src/alpha", SessionState::Done, "claude", None), 1, Some(false)),
    ]
}

/// A session with a running and a stopped CLI sub-agent, plus an orch worker running
/// in a live tmux session (`ORCH_TMUX=1`).
pub(crate) fn fixture_children_rows() -> Vec<TreeRow> {
    let subagent = |id: &str, label: &str, state| TreeRow::Child {
        row: ChildRow {
            id: id.into(),
            label: label.into(),
            kind: ChildKind::Subagent,
            state,
            preview: PreviewSource::None,
        },
        depth: 2,
        project_path: "/src/alpha".into(),
    };
    vec![
        TreeRow::Project {
            key: "alpha".into(),
            name: "alpha".into(),
            path: "/src/alpha".into(),
            session_count: 1,
            worker_count: 1,
            expanded: true,
        },
        TreeRow::Child {
            row: ChildRow::from_orch_worker(
                &OrchWorkerStatus {
                    id: "9".into(),
                    phase: "agent".into(),
                    message: "iter 1/10".into(),
                    tmux_session: Some("orch-run1-9".into()),
                    log: Some("/src/alpha/.orch/run/9.log".into()),
                    ..Default::default()
                },
                true,
            ),
            depth: 1,
            project_path: "/src/alpha".into(),
        },
        row(session("alpha-main", "/src/alpha", SessionState::Active, "claude", None), 1, Some(true)),
        subagent("a1b2c3d4e5f6", "Explore", SessionState::Active),
        subagent("f6e5d4c3b2a1", "cda-review", SessionState::Done),
    ]
}

pub(crate) fn fixture_graph() -> OrchGraphView {
    let graph: OrchGraphFile = serde_json::from_str(
        r#"{"run_id":"run-001","repo":"/src/alpha","updated_at":"2026-01-01T00:00:00Z",
        "status":"running",
        "nodes":[
          {"id":"w1","kind":"worker","phase":"implement","status":"done","message":"ok"},
          {"id":"w2","kind":"worker","phase":"implement","status":"running","message":"writing the parser tests"},
          {"id":"integrator","kind":"integrator","phase":"synthesize","status":"pending","message":"waits for workers"}],
        "edges":[{"from":"w1","to":"integrator","type":"feeds"},{"from":"w2","to":"integrator","type":"feeds"}]}"#,
    )
    .unwrap();
    OrchGraphView {
        text: cdamux_core::format_orch_graph_panel_ex(&graph, 80, Some(5)),
        // Fixed header: the live one carries the file age.
        header: "run run-001 · running".into(),
        stale: false,
        graph: Some(graph),
        workers: Vec::new(),
    }
}

/// Graph with a `worker-7` node tied to a status.json worker, for the step bar.
/// The worker is blocked so no step is still running: durations never read the clock.
pub(crate) fn fixture_steps_graph() -> OrchGraphView {
    let graph: OrchGraphFile = serde_json::from_str(
        r#"{"run_id":"run-002","repo":"/src/alpha","updated_at":"2026-01-01T00:00:00Z",
        "status":"running",
        "nodes":[
          {"id":"worker-7","kind":"worker","phase":"ci","status":"blocked","message":"CI red on lint"},
          {"id":"worker-7-grounding","kind":"verifier","phase":"grounding","status":"done","message":"ok","parent":"worker-7"}],
        "edges":[]}"#,
    )
    .unwrap();
    let worker = serde_json::from_str(
        r#"{"id":"7","phase":"blocked","message":"CI red on lint","updated_at":"t","phases":[
          {"phase":"agent","started_at":100},{"phase":"verify","started_at":220},
          {"phase":"ci","started_at":230},{"phase":"blocked","started_at":290}]}"#,
    )
    .unwrap();
    OrchGraphView {
        text: cdamux_core::format_orch_graph_panel_ex(&graph, 80, Some(5)),
        header: "run run-002 · running".into(),
        stale: false,
        graph: Some(graph),
        workers: vec![worker],
    }
}

const PREVIEW: &str = "$ cargo test\n   Compiling alpha v0.1.0\nrunning 3 tests\ntest parse ... ok\ntest render ... ok\ntest send ... ok\n\ntest result: ok. 3 passed";

/// Full-screen render of `tab` (Fleet side panel = `side_tab`) with the fixed fixtures.
pub(crate) fn render_app(w: u16, h: u16, tab: AppTab, side_tab: SideTab) -> Vec<String> {
    render_app_ex(w, h, tab, side_tab, fixture_rows(), fixture_graph(), None)
}

/// [`render_app`] with `rows`, `graph` and the full-screen Graph view open on node `graph_view`.
pub(crate) fn render_app_ex(
    w: u16,
    h: u16,
    tab: AppTab,
    side_tab: SideTab,
    rows: Vec<TreeRow>,
    graph: OrchGraphView,
    graph_view: Option<usize>,
) -> Vec<String> {
    let chrome = SessionChrome {
        repo: "alpha".into(),
        worktree: "main".into(),
        branch: "main".into(),
        agent: "claude".into(),
        uptime: "12m".into(),
        context: "40%".into(),
        tasks: "2/5".into(),
        issue: None,
        cwd: "/src/alpha".into(),
    };
    let mut cfg = Config::default();
    cfg.projects.roots.push(ProjectRoot { path: "/src".into(), label: "src".into() });
    let settings = SettingsForm::new(None);
    let state = DrawState {
        rows: &rows,
        selected_row: 1,
        side_tab,
        focus: FocusArea::Tree,
        preview_text: PREVIEW,
        preview_scroll: 0,
        notes_text: "(no notes)",
        review_text: "Select a session.",
        orch_graph: &graph,
        graph_view,
        composer_line: "",
        composer_session: Some("alpha-main"),
        session_chrome: Some(&chrome),
        status_line: "",
    };
    let (mut projects_scroll, mut sessions_scroll) = (0, 0);
    let (mut tree_rect, mut preview_rect) = (None, None);
    render(w, h, |f| {
        draw_app_screen(
            f,
            tab,
            state,
            &cfg,
            &settings,
            "/cfg/cdamux.toml",
            &[],
            &mut projects_scroll,
            &[],
            0,
            &mut sessions_scroll,
            &[],
            false,
            false,
            &mut tree_rect,
            &mut preview_rect,
        )
    })
}

pub(crate) fn render_spawn_modal(w: u16, h: u16) -> Vec<String> {
    let paths = ["/src/alpha", "/src/beta", "/src/gamma"].map(PathBuf::from).to_vec();
    let mut state = ModalState::fixture(paths, vec!["claude".into(), "cursor".into()]);
    render(w, h, |f| draw_modal(f, &mut state))
}

#[test]
fn golden_screens() {
    for (w, h) in SIZES {
        let screens = [
            ("fleet", render_app(w, h, AppTab::Fleet, SideTab::Preview)),
            ("fleet-orch-graph", render_app(w, h, AppTab::Fleet, SideTab::OrchGraph)),
            ("graph-view", render_app_ex(w, h, AppTab::Fleet, SideTab::Preview, fixture_rows(), fixture_graph(), Some(1))),
            (
                "graph-steps",
                render_app_ex(w, h, AppTab::Fleet, SideTab::Preview, fixture_rows(), fixture_steps_graph(), Some(0)),
            ),
            (
                "fleet-nested",
                render_app_ex(w, h, AppTab::Fleet, SideTab::Preview, fixture_nested_rows(), fixture_graph(), None),
            ),
            (
                "fleet-children",
                render_app_ex(w, h, AppTab::Fleet, SideTab::Preview, fixture_children_rows(), fixture_graph(), None),
            ),
            ("spawn-modal", render_spawn_modal(w, h)),
            ("settings", render_app(w, h, AppTab::Settings, SideTab::Preview)),
        ];
        for (name, lines) in screens {
            assert_snapshot(&format!("{name}-{w}x{h}"), &lines);
        }
    }
}
