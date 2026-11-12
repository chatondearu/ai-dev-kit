//! Session links: parent/child nesting of tmux sessions (`@cdamux_parent`) and the
//! generic [`ChildRow`] that non-session children (orch workers, CLI sub-agents)
//! render through in the Fleet tree.

use crate::msg::inbox_badges;
use crate::orch_graph::OrchGraphRunSummary;
use crate::orch_status::OrchWorkerStatus;
use crate::session::{EnrichedSession, SessionState};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Where a child row's preview text comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSource {
    /// `tmux capture-pane` target (a session name).
    Pane(String),
    /// Tail of a log or transcript file.
    FileTail(PathBuf),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildKind {
    /// Orch worker from `$CDAMUX_HOME/agents` (role=worker).
    Worker,
    /// CLI sub-agent from the session's ledger (`docs/subagents.md`).
    Subagent,
    /// Live orch run (`$CDAMUX_HOME/runs/<run>.json`) under the session that started it.
    Run,
}

/// A non-session row nested under a project or session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRow {
    pub id: String,
    pub label: String,
    pub kind: ChildKind,
    pub state: SessionState,
    pub preview: PreviewSource,
    /// `$CDAMUX_HOME/agents/<id>.json` stem when known (orch workers).
    pub agent_id: Option<String>,
    /// Unread inbox count for [`Self::agent_id`].
    pub unread: usize,
    /// Unanswered `ask` still in the parent's inbox (any cursor position).
    pub pending_ask: bool,
}

impl ChildRow {
    /// `session_live`: whether `w.tmux_session` is a live tmux session. Preview is
    /// that pane while it lives, else the tail of the worker log.
    pub fn from_orch_worker(w: &OrchWorkerStatus, session_live: bool) -> Self {
        let state = match w.phase.as_str() {
            "done" => SessionState::Done,
            "blocked" => SessionState::Blocked,
            "pending" => SessionState::Idle,
            _ => SessionState::Working,
        };
        let mut label = if w.message.is_empty() {
            w.phase.clone()
        } else {
            format!("{}  {}", w.phase, w.message)
        };
        if let Some(tag) = w.agent_tag() {
            label.push_str(&format!("  [{tag}]"));
        }
        let agent_id = w.agent_id.clone();
        let (unread, pending_ask) = agent_id
            .as_deref()
            .map(inbox_badges)
            .unwrap_or((0, false));
        Self {
            id: w.id.clone(),
            label,
            kind: ChildKind::Worker,
            state,
            preview: match (&w.tmux_session, &w.log) {
                (Some(s), _) if session_live => PreviewSource::Pane(s.clone()),
                (_, Some(log)) => PreviewSource::FileTail(PathBuf::from(log)),
                _ => PreviewSource::None,
            },
            agent_id,
            unread,
            pending_ask,
        }
    }
}

impl ChildRow {
    /// Run row: `id` = run id, `label` = run status.
    pub fn from_orch_run(run: &OrchGraphRunSummary) -> Self {
        let status = run.status.as_deref().filter(|s| !s.is_empty()).unwrap_or("running");
        let state = match status {
            "ok" | "done" => SessionState::Done,
            "blocked" => SessionState::Blocked,
            _ => SessionState::Working,
        };
        Self {
            id: run.run_id.clone(),
            label: status.to_string(),
            kind: ChildKind::Run,
            state,
            preview: PreviewSource::None,
            agent_id: None,
            unread: 0,
            pending_ask: false,
        }
    }
}

/// Last `max_bytes` of a file, starting at a line boundary, so a huge log or
/// transcript never lands in memory. The one bounded tail read for previews.
pub(crate) fn read_tail(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let start = f.metadata()?.len().saturating_sub(max_bytes);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.take(max_bytes).read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    // Seeking mid-file lands inside a line: drop that partial first line.
    Ok(if start > 0 {
        text.split_once('\n').map_or(String::new(), |(_, rest)| rest.to_string())
    } else {
        text
    })
}

/// Last `max_lines` lines of a log file, from its final 64 KiB.
pub fn tail_lines(path: &Path, max_lines: usize) -> std::io::Result<String> {
    let text = read_tail(path, 64 * 1024)?;
    let lines: Vec<&str> = text.lines().collect();
    Ok(lines[lines.len().saturating_sub(max_lines)..].join("\n"))
}

/// Sessions arranged by `@cdamux_parent`: indices into the input slice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionForest {
    pub roots: Vec<usize>,
    /// `children[i]` = sessions whose parent is `sessions[i]`, in input order.
    pub children: Vec<Vec<usize>>,
}

/// Nest `sessions` under their parent when the parent is in the same slice.
/// A dead parent (not in the slice) or a parent cycle makes the session a root,
/// so every session appears exactly once.
pub fn session_forest(sessions: &[EnrichedSession]) -> SessionForest {
    let index: HashMap<&str, usize> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.as_str(), i))
        .collect();
    let parent_of = |i: usize| -> Option<usize> {
        let p = index.get(sessions[i].parent.as_deref()?).copied()?;
        (p != i).then_some(p)
    };
    let mut forest = SessionForest {
        roots: Vec::new(),
        children: vec![Vec::new(); sessions.len()],
    };
    for i in 0..sessions.len() {
        // Walk up at most len steps: reaching `i` again means `i` sits on a cycle.
        let mut cur = parent_of(i);
        let mut on_cycle = false;
        for _ in 0..sessions.len() {
            match cur {
                Some(c) if c == i => {
                    on_cycle = true;
                    break;
                }
                Some(c) => cur = parent_of(c),
                None => break,
            }
        }
        match parent_of(i) {
            Some(p) if !on_cycle => forest.children[p].push(i),
            _ => forest.roots.push(i),
        }
    }
    forest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess(name: &str, parent: Option<&str>) -> EnrichedSession {
        EnrichedSession {
            name: name.into(),
            parent: parent.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn nests_three_levels() {
        let s = [sess("a", None), sess("b", Some("a")), sess("c", Some("b"))];
        let f = session_forest(&s);
        assert_eq!(f.roots, vec![0]);
        assert_eq!(f.children, vec![vec![1], vec![2], vec![]]);
    }

    #[test]
    fn dead_parent_and_self_parent_are_roots() {
        let s = [sess("a", Some("gone")), sess("b", Some("b"))];
        assert_eq!(session_forest(&s).roots, vec![0, 1]);
    }

    #[test]
    fn cycle_members_become_roots_and_keep_their_children() {
        // a <-> b cycle, c hangs off a.
        let s = [sess("a", Some("b")), sess("b", Some("a")), sess("c", Some("a"))];
        let f = session_forest(&s);
        assert_eq!(f.roots, vec![0, 1]);
        assert_eq!(f.children[0], vec![2]);
        assert!(f.children[1].is_empty());
    }

    #[test]
    fn worker_phase_maps_to_state() {
        let w = OrchWorkerStatus {
            id: "8".into(),
            phase: "blocked".into(),
            message: "waiting on review".into(),
            ..Default::default()
        };
        let row = ChildRow::from_orch_worker(&w, false);
        assert_eq!(row.state, SessionState::Blocked);
        assert_eq!(row.label, "blocked  waiting on review");
        assert_eq!(row.preview, PreviewSource::None);
    }

    #[test]
    fn worker_label_shows_cli_and_tier() {
        let w = OrchWorkerStatus {
            id: "8".into(),
            phase: "agent".into(),
            message: "iter 1/10".into(),
            cli: Some("claude".into()),
            tier: Some("hard".into()),
            ..Default::default()
        };
        assert_eq!(ChildRow::from_orch_worker(&w, false).label, "agent  iter 1/10  [claude·hard]");
    }

    #[test]
    fn worker_preview_pane_while_live_else_log() {
        let w = OrchWorkerStatus {
            id: "8".into(),
            tmux_session: Some("orch-r-8".into()),
            log: Some("/r/.orch/run/8.log".into()),
            ..Default::default()
        };
        assert_eq!(
            ChildRow::from_orch_worker(&w, true).preview,
            PreviewSource::Pane("orch-r-8".into())
        );
        assert_eq!(
            ChildRow::from_orch_worker(&w, false).preview,
            PreviewSource::FileTail("/r/.orch/run/8.log".into())
        );
    }

    #[test]
    fn tail_lines_reads_only_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("big.log");
        let body: String = (0..20_000).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&p, body).unwrap();
        assert_eq!(tail_lines(&p, 2).unwrap(), "line 19998\nline 19999");
        std::fs::write(&p, "a\nb\n").unwrap();
        assert_eq!(tail_lines(&p, 10).unwrap(), "a\nb");
    }
}
