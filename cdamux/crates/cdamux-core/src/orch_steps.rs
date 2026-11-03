//! A worker's pipeline as steps (implement → verify → review → CI → PR) with live
//! state and durations, from the phases `lib/progress.sh` writes to `status.json`.

use crate::orch_graph::OrchGraphNode;
use crate::orch_graph_layout::NodeState;
use crate::orch_status::OrchWorkerStatus;

/// Step label and the harness phase that runs it (`done` = PR open and green).
const PIPELINE: [(&str, &str); 5] = [
    ("implement", "agent"),
    ("verify", "verify"),
    ("review", "review"),
    ("CI", "ci"),
    ("PR", "done"),
];
/// Harness phases that are not a step: before the pipeline, or a terminal state.
const NON_STEP: [&str; 3] = ["pending", "worktree", "blocked"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub state: NodeState,
    pub secs: Option<u64>,
}

impl Step {
    /// `✓ verify 14s`
    pub fn text(&self) -> String {
        match self.secs {
            Some(s) => format!("{} {} {}", self.state.icon(), self.label, format_secs(s)),
            None => format!("{} {}", self.state.icon(), self.label),
        }
    }
}

fn format_secs(s: u64) -> String {
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m{:02}s", s / 60, s % 60),
        _ => format!("{}h{:02}m", s / 3600, s % 3600 / 60),
    }
}

/// Seconds spent in `phase`, summed over every visit (loops re-enter agent/verify).
fn phase_secs(w: &OrchWorkerStatus, phase: &str, now: u64) -> Option<u64> {
    let mut total = None;
    for (i, m) in w.phases.iter().enumerate().filter(|(_, m)| m.phase == phase) {
        let end = match w.phases.get(i + 1) {
            Some(next) => next.started_at,
            // The last mark is still running unless it is a terminal phase.
            None if phase == "done" || phase == "blocked" => continue,
            None => now,
        };
        *total.get_or_insert(0) += end.saturating_sub(m.started_at);
    }
    total
}

/// Steps for `w`. `verifier`: state of the grounding-verifier graph node, shown
/// after verify when the run has one. A phase the pipeline does not know is
/// appended verbatim as the running step.
pub fn worker_steps(w: &OrchWorkerStatus, verifier: Option<NodeState>, now: u64) -> Vec<Step> {
    let cur = w.phase.as_str();
    let cur_idx = PIPELINE.iter().position(|(_, p)| *p == cur);
    let blocked = cur == "blocked";
    // Blocked overwrites the phase: the step that blocked is the last real one in history.
    let active = if blocked {
        w.phases
            .iter()
            .rev()
            .map(|m| m.phase.as_str())
            .find(|p| !NON_STEP.contains(p) && *p != "done")
    } else {
        Some(cur)
    };
    let mut steps: Vec<Step> = PIPELINE
        .iter()
        .enumerate()
        .map(|(i, (label, phase))| {
            let seen = w.phases.iter().any(|m| m.phase == *phase);
            let state = if *phase == "done" {
                if cur == "done" { NodeState::Done } else { NodeState::Queued }
            } else if active == Some(*phase) {
                if blocked { NodeState::Blocked } else { NodeState::Running }
            } else if seen || cur_idx.is_some_and(|c| i < c) {
                NodeState::Done
            } else {
                NodeState::Queued
            };
            Step { label: (*label).into(), state, secs: phase_secs(w, phase, now) }
        })
        .collect();
    if let Some(state) = verifier {
        steps.insert(2, Step { label: "verifier".into(), state, secs: None });
    }
    if cur_idx.is_none() && !NON_STEP.contains(&cur) && !cur.is_empty() {
        steps.push(Step { label: cur.into(), state: NodeState::Running, secs: phase_secs(w, cur, now) });
    }
    steps
}

/// Status worker id (`162`) for a graph node: a `worker-162` node or one of its children.
pub fn step_worker_id(node: &OrchGraphNode) -> Option<&str> {
    let name = if node.kind == "worker" { Some(node.id.as_str()) } else { node.parent.as_deref() };
    name?.strip_prefix("worker-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orch_status::PhaseMark;

    fn worker(phase: &str, marks: &[(&str, u64)]) -> OrchWorkerStatus {
        OrchWorkerStatus {
            id: "1".into(),
            phase: phase.into(),
            message: String::new(),
            updated_at: String::new(),
            phases: marks.iter().map(|(p, t)| PhaseMark { phase: (*p).into(), started_at: *t }).collect(),
            ..Default::default()
        }
    }

    fn states(steps: &[Step]) -> Vec<(&str, NodeState)> {
        steps.iter().map(|s| (s.label.as_str(), s.state)).collect()
    }

    #[test]
    fn maps_phases_to_states_and_durations() {
        use NodeState::*;
        // agent 0-30, verify 30-44, agent again 44-60, verify running since 60 (now 75).
        let w = worker("verify", &[("pending", 0), ("agent", 0), ("verify", 30), ("agent", 44), ("verify", 60)]);
        let steps = worker_steps(&w, None, 75);
        assert_eq!(
            states(&steps),
            [("implement", Done), ("verify", Running), ("review", Queued), ("CI", Queued), ("PR", Queued)]
        );
        assert_eq!(steps[0].secs, Some(46));
        assert_eq!(steps[1].secs, Some(14 + 15));
        assert_eq!(steps[2].secs, None);
        assert_eq!(steps[1].text(), "◎ verify 29s");
    }

    #[test]
    fn done_blocked_unknown_and_legacy() {
        use NodeState::*;
        let done = worker_steps(&worker("done", &[("agent", 0), ("ci", 10), ("done", 20)]), None, 99);
        assert!(done.iter().all(|s| s.state == Done));
        assert_eq!(done[3].secs, Some(10));
        assert_eq!(done[4].secs, None);

        // The phase after blocked is lost: history says which step blocked.
        let b = worker_steps(&worker("blocked", &[("agent", 0), ("ci", 10), ("blocked", 20)]), None, 99);
        assert_eq!(states(&b)[3], ("CI", Blocked));
        assert_eq!(states(&b)[0], ("implement", Done));
        assert_eq!(b[3].secs, Some(10));

        // No history (older file): earlier steps by order, no durations.
        let legacy = worker_steps(&worker("ci", &[]), None, 99);
        assert_eq!(states(&legacy)[..4], [("implement", Done), ("verify", Done), ("review", Done), ("CI", Running)]);
        assert!(legacy.iter().all(|s| s.secs.is_none()));

        let unknown = worker_steps(&worker("merging", &[("merging", 5)]), Some(Running), 9);
        assert_eq!(unknown.last().map(|s| (s.label.as_str(), s.state, s.secs)), Some(("merging", Running, Some(4))));
        assert_eq!(unknown[2].label, "verifier");

        let early = worker_steps(&worker("worktree", &[]), None, 0);
        assert!(early.iter().all(|s| s.state == Queued) && early.len() == 5);
    }

    #[test]
    fn graph_node_maps_to_worker_id() {
        let node = |id: &str, kind: &str, parent: Option<&str>| OrchGraphNode {
            id: id.into(),
            kind: kind.into(),
            phase: String::new(),
            status: String::new(),
            message: String::new(),
            parent: parent.map(Into::into),
        };
        assert_eq!(step_worker_id(&node("worker-162", "worker", None)), Some("162"));
        assert_eq!(step_worker_id(&node("worker-162-verify-1", "verify", Some("worker-162"))), Some("162"));
        assert_eq!(step_worker_id(&node("integrator", "integrator", None)), None);
    }
}
