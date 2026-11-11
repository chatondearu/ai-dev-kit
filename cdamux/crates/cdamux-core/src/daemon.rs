//! Session list for `cdamux list` and the TUI: tmux is read directly.

use crate::agent_status::DEFAULT_WORKING_WARN_SECONDS;
use crate::session::{enrich_sessions_ex, EnrichedSession};
use crate::{group_sessions, list_tmux_sessions, GroupBy, ProjectsConfig};

pub fn sessions_for_list(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> Vec<EnrichedSession> {
    sessions_for_list_ex(
        group_by,
        projects,
        idle_seconds_for_needs_input,
        DEFAULT_WORKING_WARN_SECONDS,
    )
}

pub fn sessions_for_list_ex(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
) -> Vec<EnrichedSession> {
    let raw = list_tmux_sessions().unwrap_or_default();
    enrich_sessions_ex(
        &raw,
        group_by,
        projects,
        idle_seconds_for_needs_input,
        working_warn_seconds,
    )
}

pub fn grouped_for_list(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> crate::session::GroupedSessions {
    let sessions = sessions_for_list(group_by, projects, idle_seconds_for_needs_input);
    group_sessions(sessions, group_by)
}

pub fn grouped_for_list_ex(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
) -> crate::session::GroupedSessions {
    let sessions = sessions_for_list_ex(
        group_by,
        projects,
        idle_seconds_for_needs_input,
        working_warn_seconds,
    );
    group_sessions(sessions, group_by)
}
