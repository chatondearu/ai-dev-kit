//! Session links: parent/child nesting of tmux sessions (`@cdamux_parent`) and the
//! generic [`ChildRow`] that non-session children (orch workers, later sub-agents)
//! render through in the Fleet tree.

use crate::orch_status::OrchWorkerStatus;
use crate::session::{EnrichedSession, SessionState};
use std::collections::HashMap;
use std::path::PathBuf;

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
    /// Headless orch worker from `.orch/run/status.json`.
    Worker,
}

/// A non-session row nested under a project or session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRow {
    pub id: String,
    pub label: String,
    pub kind: ChildKind,
    pub state: SessionState,
    pub preview: PreviewSource,
}

impl ChildRow {
    pub fn from_orch_worker(w: &OrchWorkerStatus) -> Self {
        let state = match w.phase.as_str() {
            "done" => SessionState::Done,
            "blocked" => SessionState::Error,
            "pending" => SessionState::Idle,
            _ => SessionState::Active,
        };
        let label = if w.message.is_empty() {
            w.phase.clone()
        } else {
            format!("{}  {}", w.phase, w.message)
        };
        Self {
            id: w.id.clone(),
            label,
            kind: ChildKind::Worker,
            state,
            preview: PreviewSource::None,
        }
    }
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
            updated_at: String::new(),
            phases: Vec::new(),
        };
        let row = ChildRow::from_orch_worker(&w);
        assert_eq!(row.state, SessionState::Error);
        assert_eq!(row.label, "blocked  waiting on review");
        assert_eq!(row.preview, PreviewSource::None);
    }
}
