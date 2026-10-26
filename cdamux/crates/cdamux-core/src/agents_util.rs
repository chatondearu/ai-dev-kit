//! Agent availability on PATH (spawn defaults).

use crate::{configured_agent_ids, resolve_agent_command, AgentsConfig};
use std::process::Command;

pub fn command_on_path(name: &str) -> bool {
    let bin = name.split_whitespace().next().unwrap_or(name);
    let escaped = if bin
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
    {
        bin.to_string()
    } else {
        format!("'{}'", bin.replace('\'', "'\\''"))
    };
    Command::new("sh")
        .args(["-c", &format!("command -v -- {}", escaped)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn agent_available(agents: &AgentsConfig, id: &str) -> bool {
    resolve_agent_command(agents, id)
        .map(|c| command_on_path(&c))
        .unwrap_or(false)
}

/// Config `[agents].default` when on PATH, else first configured agent on PATH.
pub fn preferred_spawn_agent(agents: &AgentsConfig) -> String {
    if agent_available(agents, &agents.default) {
        return agents.default.clone();
    }
    for id in configured_agent_ids(agents) {
        if agent_available(agents, &id) {
            return id;
        }
    }
    agents.default.clone()
}

pub fn preferred_spawn_agent_index(agents: &AgentsConfig, ids: &[String]) -> usize {
    let pref = preferred_spawn_agent(agents);
    ids.iter().position(|s| s == &pref).unwrap_or(0)
}
