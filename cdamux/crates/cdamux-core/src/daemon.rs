//! Daemon socket path and JSON-line IPC client.

use crate::session::EnrichedSession;
use crate::{enrich_sessions, group_sessions, list_tmux_sessions, GroupBy, ProjectsConfig};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon socket not available")]
    Unavailable,
    #[error("daemon IPC: {0}")]
    Io(String),
    #[error("daemon response: {0}")]
    Protocol(String),
}

#[derive(Debug, Deserialize)]
struct DaemonRequest {
    op: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonListResponse {
    pub sessions: Vec<EnrichedSession>,
}

pub fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
}

pub fn socket_path() -> Option<PathBuf> {
    runtime_dir().map(|r| r.join("cdamux").join("cdamux.sock"))
}

pub fn state_cache_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .map(|p| p.join("cdamux").join("sessions.json"))
}

/// Poll tmux and build enriched session list (shared by daemon and tests).
pub fn poll_sessions(
    projects: &ProjectsConfig,
    group_by: GroupBy,
    idle_seconds_for_needs_input: u32,
) -> Vec<EnrichedSession> {
    let raw = list_tmux_sessions().unwrap_or_default();
    enrich_sessions(&raw, group_by, projects, idle_seconds_for_needs_input)
}

/// Write latest poll snapshot for CLI fallback without a running daemon.
pub fn write_state_cache(sessions: &[EnrichedSession]) -> std::io::Result<()> {
    let path = state_cache_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no cache path")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(sessions)?;
    std::fs::write(path, payload)
}

pub fn read_state_cache() -> Option<Vec<EnrichedSession>> {
    let path = match state_cache_path() {
        Some(p) => p,
        None => return None,
    };
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Ask cdamuxd for session list; returns None if socket missing or daemon down.
pub fn daemon_list_sessions() -> Result<Vec<EnrichedSession>, DaemonError> {
    let path = socket_path().ok_or(DaemonError::Unavailable)?;
    if !path.exists() {
        return Err(DaemonError::Unavailable);
    }
    let mut stream = UnixStream::connect(&path).map_err(|e| DaemonError::Io(e.to_string()))?;
    let req = r#"{"op":"list"}"#;
    stream
        .write_all(req.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|e| DaemonError::Io(e.to_string()))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| DaemonError::Io(e.to_string()))?;
    let resp: DaemonListResponse =
        serde_json::from_str(line.trim()).map_err(|e| DaemonError::Protocol(e.to_string()))?;
    Ok(resp.sessions)
}

/// Handle one IPC line (daemon side).
pub fn handle_daemon_request(
    line: &str,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> String {
    let req: DaemonRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({ "error": e.to_string() }).to_string();
        }
    };
    match req.op.as_str() {
        "list" => {
            let sessions = poll_sessions(projects, GroupBy::Project, idle_seconds_for_needs_input);
            let resp = DaemonListResponse { sessions };
            serde_json::to_string(&resp).unwrap_or_else(|e| {
                serde_json::json!({ "error": e.to_string() }).to_string()
            })
        }
        "health" => serde_json::json!({ "ok": true, "service": "cdamuxd" }).to_string(),
        _ => serde_json::json!({ "error": "unknown op" }).to_string(),
    }
}

/// Merge daemon state into grouped list when available.
pub fn sessions_for_list(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> Vec<EnrichedSession> {
    if let Ok(daemon) = daemon_list_sessions() {
        if !daemon.is_empty() {
            return rekey_daemon_sessions(daemon, group_by, projects, idle_seconds_for_needs_input);
        }
    }
    if let Some(cached) = read_state_cache() {
        if !cached.is_empty() {
            return rekey_daemon_sessions(cached, group_by, projects, idle_seconds_for_needs_input);
        }
    }
    let raw = list_tmux_sessions().unwrap_or_default();
    enrich_sessions(&raw, group_by, projects, idle_seconds_for_needs_input)
}

fn rekey_daemon_sessions(
    sessions: Vec<EnrichedSession>,
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> Vec<EnrichedSession> {
    let raw: Vec<crate::TmuxSession> = sessions
        .iter()
        .map(|s| crate::TmuxSession {
            name: s.name.clone(),
            path: s.path.clone(),
        })
        .collect();
    enrich_sessions(&raw, group_by, projects, idle_seconds_for_needs_input)
        .into_iter()
        .zip(sessions)
        .map(|(mut e, old)| {
            e.state = old.state;
            e
        })
        .collect()
}

pub fn grouped_for_list(
    group_by: GroupBy,
    projects: &ProjectsConfig,
    idle_seconds_for_needs_input: u32,
) -> crate::session::GroupedSessions {
    let sessions = sessions_for_list(group_by, projects, idle_seconds_for_needs_input);
    group_sessions(sessions, group_by)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectsConfig;

    #[test]
    fn health_op() {
        let cfg = ProjectsConfig::default();
        let line = handle_daemon_request(r#"{"op":"health"}"#, &cfg, 3);
        assert!(line.contains("ok"));
    }
}
