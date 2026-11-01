//! Session list for `cdamux list` and the TUI: tmux is read directly.

use crate::session::EnrichedSession;
use crate::{enrich_sessions, group_sessions, list_tmux_sessions, GroupBy, ProjectsConfig};

pub fn sessions_for_list(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> Vec<EnrichedSession> {
    let raw = list_tmux_sessions().unwrap_or_default();
    enrich_sessions(&raw, group_by, projects, idle_seconds_for_needs_input)
}

pub fn grouped_for_list(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> crate::session::GroupedSessions {
    let sessions = sessions_for_list(group_by, projects, idle_seconds_for_needs_input);
    group_sessions(sessions, group_by)
}
