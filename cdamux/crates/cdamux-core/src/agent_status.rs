//! Agent status set, precedence and stickiness (ADR-011 §3).
//! Reported status wins; pane heuristics apply when none is stored, and
//! `needs_input` from the pane can still override reported `idle`/`working`
//! (Cursor has no permission event). `done` / `error` stick until the next
//! prompt submit (`working`). A quiet `working` pane past the warn threshold
//! is shown as `working ⚠` without rewriting state.
//!
//! `may_overwrite` is used by JS hooks (`ledger.js`); Rust `state append`/`set`
//! stickiness is deferred to slice 2/3.

use crate::session::SessionState;
use crate::state::{state_get, StateKind};
use serde_json::Value;

/// Default quiet-pane threshold before `working ⚠` (10 minutes).
pub const DEFAULT_WORKING_WARN_SECONDS: u32 = 600;

/// Parse a stored / hook status string into [`SessionState`].
/// Accepts ADR-011 names plus transitional `@cdamux_state` aliases.
pub fn parse_reported_status(raw: &str) -> Option<SessionState> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "working" | "running" | "active" | "busy" => Some(SessionState::Working),
        "idle" => Some(SessionState::Idle),
        "needs_input" | "needs-input" | "waiting" => Some(SessionState::NeedsInput),
        "blocked" => Some(SessionState::Blocked),
        "done" => Some(SessionState::Done),
        "error" | "failed" => Some(SessionState::Error),
        "dormant" => Some(SessionState::Dormant),
        _ => None,
    }
}

/// Sticky statuses that heuristics / idle hooks must not clear.
pub fn is_sticky(status: SessionState) -> bool {
    matches!(status, SessionState::Done | SessionState::Error)
}

/// Whether a writer may replace `current` with `next` (stickiness).
pub fn may_overwrite(current: Option<SessionState>, next: SessionState) -> bool {
    match current {
        Some(cur) if is_sticky(cur) => next == SessionState::Working,
        _ => true,
    }
}

/// Read `status` from `agents/<id>.json` when present.
pub fn load_reported_status(agent_id: &str) -> Option<SessionState> {
    let value = state_get(StateKind::Agents, agent_id).ok().flatten()?;
    status_from_value(&value)
}

fn status_from_value(value: &Value) -> Option<SessionState> {
    value
        .get("status")
        .and_then(|v| v.as_str())
        .and_then(parse_reported_status)
}

/// Whether pane heuristics should still run for this reported status.
/// `idle`/`working` keep the Cursor `needs_input` path (ADR-011 §3).
pub fn heuristic_applies(reported: Option<SessionState>) -> bool {
    match reported {
        None => true,
        Some(SessionState::Idle | SessionState::Working) => true,
        Some(_) => false,
    }
}

/// Resolve display status for an agent.
///
/// Precedence: no live host → `dormant`; else reported (file / hook) wins,
/// except pane `needs_input` overrides reported `idle`/`working`; else pane
/// heuristic. When reported is `working` and the pane has been quiet longer
/// than `working_warn_seconds`, `working_stale` is true (display only).
pub fn resolve_display_status(
    host_alive: bool,
    reported: Option<SessionState>,
    heuristic: SessionState,
    idle_seconds: Option<u32>,
    working_warn_seconds: u32,
) -> (SessionState, bool) {
    if !host_alive {
        return (SessionState::Dormant, false);
    }
    let state = match reported {
        Some(SessionState::Idle | SessionState::Working)
            if heuristic == SessionState::NeedsInput =>
        {
            SessionState::NeedsInput
        }
        Some(s) => s,
        None => heuristic,
    };
    let warn = working_warn_seconds.max(1);
    let stale = state == SessionState::Working
        && idle_seconds.is_some_and(|idle| idle >= warn);
    (state, stale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::state::{initial_agent_value, state_append, state_set};
    use std::path::Path;

    fn with_temp_home<F: FnOnce(&Path)>(f: F) {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn parse_seven_statuses_and_aliases() {
        assert_eq!(parse_reported_status("working"), Some(SessionState::Working));
        assert_eq!(parse_reported_status("running"), Some(SessionState::Working));
        assert_eq!(parse_reported_status("idle"), Some(SessionState::Idle));
        assert_eq!(
            parse_reported_status("needs_input"),
            Some(SessionState::NeedsInput)
        );
        assert_eq!(parse_reported_status("blocked"), Some(SessionState::Blocked));
        assert_eq!(parse_reported_status("done"), Some(SessionState::Done));
        assert_eq!(parse_reported_status("error"), Some(SessionState::Error));
        assert_eq!(parse_reported_status("dormant"), Some(SessionState::Dormant));
        assert_eq!(parse_reported_status("nope"), None);
    }

    #[test]
    fn reported_wins_over_heuristic() {
        let (s, stale) = resolve_display_status(
            true,
            Some(SessionState::Idle),
            SessionState::Working,
            Some(0),
            600,
        );
        assert_eq!(s, SessionState::Idle);
        assert!(!stale);
    }

    #[test]
    fn needs_input_heuristic_overrides_idle_and_working() {
        let (s, stale) = resolve_display_status(
            true,
            Some(SessionState::Idle),
            SessionState::NeedsInput,
            Some(10),
            600,
        );
        assert_eq!(s, SessionState::NeedsInput);
        assert!(!stale);

        let (s2, stale2) = resolve_display_status(
            true,
            Some(SessionState::Working),
            SessionState::NeedsInput,
            Some(601),
            600,
        );
        assert_eq!(s2, SessionState::NeedsInput);
        assert!(!stale2);
    }

    #[test]
    fn sticky_reported_ignores_needs_input_heuristic() {
        for sticky in [
            SessionState::Done,
            SessionState::Error,
            SessionState::Blocked,
        ] {
            let (s, _) = resolve_display_status(
                true,
                Some(sticky),
                SessionState::NeedsInput,
                Some(10),
                600,
            );
            assert_eq!(s, sticky);
            assert!(!heuristic_applies(Some(sticky)));
        }
        assert!(heuristic_applies(None));
        assert!(heuristic_applies(Some(SessionState::Idle)));
        assert!(heuristic_applies(Some(SessionState::Working)));
    }

    #[test]
    fn heuristic_when_no_report() {
        let (s, _) = resolve_display_status(
            true,
            None,
            SessionState::NeedsInput,
            Some(10),
            600,
        );
        assert_eq!(s, SessionState::NeedsInput);
    }

    #[test]
    fn sticky_done_and_error_until_working() {
        assert!(!may_overwrite(Some(SessionState::Done), SessionState::Idle));
        assert!(!may_overwrite(Some(SessionState::Error), SessionState::NeedsInput));
        assert!(may_overwrite(Some(SessionState::Done), SessionState::Working));
        assert!(may_overwrite(Some(SessionState::Idle), SessionState::Working));
        assert!(may_overwrite(None, SessionState::Idle));
    }

    #[test]
    fn dormant_when_host_missing() {
        let (s, stale) = resolve_display_status(
            false,
            Some(SessionState::Working),
            SessionState::Idle,
            None,
            600,
        );
        assert_eq!(s, SessionState::Dormant);
        assert!(!stale);
    }

    #[test]
    fn working_warn_is_display_only() {
        let (s, stale) = resolve_display_status(
            true,
            Some(SessionState::Working),
            SessionState::Idle,
            Some(601),
            600,
        );
        assert_eq!(s, SessionState::Working);
        assert!(stale);

        let (s2, stale2) = resolve_display_status(
            true,
            Some(SessionState::Working),
            SessionState::Idle,
            Some(59),
            600,
        );
        assert_eq!(s2, SessionState::Working);
        assert!(!stale2);
    }

    #[test]
    fn load_reported_from_agent_file() {
        with_temp_home(|_| {
            let id = "a-stat1";
            state_set(
                StateKind::Agents,
                id,
                &initial_agent_value(id, "claude", "s1", "agent", None),
            )
            .unwrap();
            assert_eq!(load_reported_status(id), None);
            state_append(
                StateKind::Agents,
                id,
                &serde_json::json!({"status": "working"}),
            )
            .unwrap();
            assert_eq!(load_reported_status(id), Some(SessionState::Working));
        });
    }
}
