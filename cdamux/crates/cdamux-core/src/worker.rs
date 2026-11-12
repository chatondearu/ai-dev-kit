//! Interactive workers under an orchestrator (`cdamux worker spawn|ls|peek`).

use crate::client_name::normalize_client_name;
use crate::paths::cdamux_state_dir;
use crate::send::send_to_session;
use crate::session_manage::unique_session_name;
use crate::spawn::{
    resolve_agent_command, session_name_for_path, spawn_tmux_session_ex, PaneLayout, SpawnError,
    SpawnExtra, SpawnOutcome,
};
use crate::state::{
    generate_agent_id, state_append, state_get, state_set, valid_state_id, StateKind,
};
use crate::tmux::capture_pane_text;
use crate::worktree::{
    current_branch, git_toplevel, resolve_worktree_dest, worktree_add_new_branch, WorktreeError,
};
use crate::AgentsConfig;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("must run inside an orchestrator (set CDAMUX_AGENT_ID)")]
    NoCaller,
    #[error("caller agent not found: {0}")]
    CallerNotFound(String),
    #[error("caller has no host (tmux session)")]
    NoHost,
    #[error("invalid worker name '{0}'")]
    BadName(String),
    #[error("worker already exists: {0}")]
    Exists(String),
    #[error("worktree root unset ([worktrees].root or $CDAMUX_HOME/worktrees)")]
    NoWorktreeRoot,
    #[error("path is not inside a git repository")]
    NotGit,
    #[error("{0}")]
    Spawn(#[from] SpawnError),
    #[error("{0}")]
    Worktree(#[from] WorktreeError),
    #[error("io: {0}")]
    Io(String),
    #[error("worker not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Other(String),
}

impl From<io::Error> for WorkerError {
    fn from(e: io::Error) -> Self {
        WorkerError::Io(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct WorkerSpawnRequest<'a> {
    pub name: &'a str,
    pub caller_id: &'a str,
    /// Product client override; inherit caller's client when `None`.
    pub agent: Option<&'a str>,
    pub prompt_file: Option<&'a Path>,
    /// Spawn in the caller's cwd (no new worktree/branch).
    pub here: bool,
    pub worktree_root: Option<&'a Path>,
    pub agents: &'a AgentsConfig,
    /// Fallback cwd when the caller's host path cannot be resolved.
    pub cwd: &'a Path,
}

#[derive(Debug, Clone)]
pub struct WorkerSpawnResult {
    pub agent_id: String,
    pub host: String,
    pub name: String,
    pub path: PathBuf,
    pub run: String,
    pub outcome: SpawnOutcome,
}

#[derive(Debug, Clone)]
pub struct WorkerRow {
    pub name: String,
    pub id: String,
    pub host: String,
    pub client: String,
    pub run: Option<String>,
    pub status: Option<String>,
}

/// Shell-escape for embedding a prompt in a pane command line.
pub fn shell_single_quote(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Build the pane launch command; opencode takes `--prompt`, others paste after spawn.
/// Returns `(command, paste_prompt_after)`.
pub fn with_first_prompt(command: &str, client: &str, prompt: Option<&str>) -> (String, bool) {
    let Some(text) = prompt.map(str::trim).filter(|s| !s.is_empty()) else {
        return (command.to_string(), false);
    };
    match normalize_client_name(client) {
        "opencode" => (
            format!("{command} --prompt {}", shell_single_quote(text)),
            false,
        ),
        _ => (command.to_string(), true),
    }
}

fn sanitize_worker_name(name: &str) -> Result<String, WorkerError> {
    let t = name.trim();
    if t.is_empty() || !valid_state_id(t) {
        return Err(WorkerError::BadName(name.to_string()));
    }
    Ok(t.to_string())
}

fn slug_session(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("c-{}", if safe.is_empty() { "worker" } else { &safe })
}

fn generate_run_id() -> Option<String> {
    generate_agent_id().map(|id| format!("r-{}", id.trim_start_matches("a-")))
}

fn list_agent_docs() -> io::Result<Vec<(String, Value)>> {
    let Some(dir) = cdamux_state_dir("agents") else {
        return Ok(Vec::new());
    };
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for ent in fs::read_dir(dir)? {
        let ent = ent?;
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
        out.push((id, v));
    }
    Ok(out)
}

/// Ensure the orchestrator has a long-lived run id (mint + write when missing).
pub fn ensure_orch_run(caller_id: &str, caller: &Value, repo: &Path) -> Result<String, WorkerError> {
    if let Some(run) = caller
        .get("run")
        .and_then(|r| r.as_str())
        .filter(|s| !s.is_empty())
    {
        return Ok(run.to_string());
    }
    let run = generate_run_id().ok_or_else(|| WorkerError::Other("cannot mint run id".into()))?;
    state_append(StateKind::Agents, caller_id, &json!({ "run": run }))?;
    let _ = state_set(
        StateKind::Runs,
        &run,
        &json!({
            "id": run,
            "parent": caller_id,
            "repo": repo.display().to_string(),
        }),
    );
    Ok(run)
}

fn find_worker_doc(key: &str, parent: Option<&str>) -> Result<(String, Value), WorkerError> {
    let docs = list_agent_docs().map_err(|e| WorkerError::Io(e.to_string()))?;
    for (id, v) in docs {
        if let Some(p) = parent {
            if v.get("role").and_then(|r| r.as_str()) != Some("worker")
                || v.get("parent").and_then(|x| x.as_str()) != Some(p)
            {
                continue;
            }
        }
        let name = v.get("name").and_then(|n| n.as_str());
        let host = v.get("host").and_then(|h| h.as_str());
        if id == key || name == Some(key) || host == Some(key) {
            return Ok((id, v));
        }
    }
    Err(WorkerError::NotFound(key.to_string()))
}

/// List workers whose `parent` is `caller_id` (or all workers when `caller_id` is empty — unused).
pub fn list_workers(caller_id: &str) -> Result<Vec<WorkerRow>, WorkerError> {
    let docs = list_agent_docs().map_err(|e| WorkerError::Io(e.to_string()))?;
    let mut rows: Vec<WorkerRow> = docs
        .into_iter()
        .filter(|(_, v)| {
            v.get("role").and_then(|r| r.as_str()) == Some("worker")
                && v.get("parent").and_then(|p| p.as_str()) == Some(caller_id)
        })
        .map(|(id, v)| WorkerRow {
            name: v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(&id)
                .to_string(),
            id,
            host: v
                .get("host")
                .and_then(|h| h.as_str())
                .unwrap_or("")
                .to_string(),
            client: v
                .get("client")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string(),
            run: v
                .get("run")
                .and_then(|r| r.as_str())
                .map(str::to_string),
            status: v
                .get("status")
                .and_then(|s| s.as_str())
                .map(str::to_string),
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

/// Capture the last `lines` of a worker pane (by name, id, or host).
pub fn peek_worker(caller_id: &str, key: &str, lines: i32) -> Result<String, WorkerError> {
    let (_id, v) = find_worker_doc(key, Some(caller_id))?;
    let host = v
        .get("host")
        .and_then(|h| h.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(WorkerError::NoHost)?;
    capture_pane_text(host, lines.max(1), false).map_err(|e| WorkerError::Other(e.to_string()))
}

fn resolve_caller_cwd(caller: &Value, fallback: &Path) -> PathBuf {
    if let Some(host) = caller.get("host").and_then(|h| h.as_str()) {
        if let Ok(sessions) = crate::tmux::list_tmux_sessions() {
            if let Some(s) = sessions.into_iter().find(|s| s.name == host) {
                if !s.path.is_empty() {
                    return PathBuf::from(s.path);
                }
            }
        }
        if let Some(rec) = crate::session_registry::load(host) {
            return rec.cwd;
        }
    }
    fallback.to_path_buf()
}

fn worker_name_taken(name: &str, caller_id: &str) -> bool {
    list_workers(caller_id)
        .ok()
        .is_some_and(|rows| rows.iter().any(|r| r.name == name || r.host == slug_session(name)))
}

/// Spawn an interactive worker under the caller's orchestrator.
pub fn spawn_worker(req: WorkerSpawnRequest<'_>) -> Result<WorkerSpawnResult, WorkerError> {
    let name = sanitize_worker_name(req.name)?;
    if worker_name_taken(&name, req.caller_id) {
        return Err(WorkerError::Exists(name));
    }

    let caller = state_get(StateKind::Agents, req.caller_id)?
        .ok_or_else(|| WorkerError::CallerNotFound(req.caller_id.to_string()))?;
    let parent_host = caller
        .get("host")
        .and_then(|h| h.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(WorkerError::NoHost)?
        .to_string();

    let client = req
        .agent
        .map(normalize_client_name)
        .map(str::to_string)
        .or_else(|| {
            caller
                .get("client")
                .and_then(|c| c.as_str())
                .map(|c| normalize_client_name(c).to_string())
        })
        .unwrap_or_else(|| normalize_client_name(&req.agents.default).to_string());

    let base_cwd = resolve_caller_cwd(&caller, req.cwd);
    let run = ensure_orch_run(req.caller_id, &caller, &base_cwd)?;

    let spawn_path = if req.here {
        base_cwd.clone()
    } else {
        let root = req
            .worktree_root
            .map(Path::to_path_buf)
            .or_else(|| cdamux_state_dir("worktrees"))
            .ok_or(WorkerError::NoWorktreeRoot)?;
        let repo = git_toplevel(&base_cwd).ok_or(WorkerError::NotGit)?;
        let base_branch = current_branch(&repo).unwrap_or_else(|| "HEAD".into());
        let branch = format!("worker/{name}");
        let dest = resolve_worktree_dest(&root, &name);
        if dest.exists() {
            return Err(WorkerError::Exists(dest.display().to_string()));
        }
        worktree_add_new_branch(&repo, &dest, &branch, &base_branch)?;
        dest
    };

    let prompt = match req.prompt_file {
        Some(p) => Some(fs::read_to_string(p).map_err(|e| WorkerError::Io(e.to_string()))?),
        None => None,
    };

    let base_cmd = resolve_agent_command(req.agents, &client)?;
    let (launch_cmd, paste_after) = with_first_prompt(&base_cmd, &client, prompt.as_deref());

    let session_base = if req.here {
        session_name_for_path(&spawn_path)
    } else {
        slug_session(&name)
    };
    let host = unique_session_name(&session_base);

    let extra = SpawnExtra {
        role: Some("worker"),
        run: Some(run.as_str()),
        parent_agent_id: Some(req.caller_id),
        name: Some(name.as_str()),
    };
    let outcome = spawn_tmux_session_ex(
        &host,
        &spawn_path,
        &launch_cmd,
        PaneLayout::Single,
        false,
        Some(client.as_str()),
        Some(parent_host.as_str()),
        extra,
    )?;

    let agent_id = crate::session_registry::load(&host)
        .and_then(|r| r.cdamux_id)
        .or_else(|| {
            list_agent_docs().ok().and_then(|docs| {
                docs.into_iter().find_map(|(id, v)| {
                    (v.get("host").and_then(|h| h.as_str()) == Some(host.as_str()))
                        .then_some(id)
                })
            })
        })
        .unwrap_or_default();

    if paste_after {
        if let Some(text) = prompt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let _ = send_to_session(&host, text, true);
        }
    }

    Ok(WorkerSpawnResult {
        agent_id,
        host,
        name,
        path: spawn_path,
        run,
        outcome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_uses_prompt_flag() {
        let (cmd, paste) = with_first_prompt("opencode", "opencode", Some("go"));
        assert!(!paste);
        assert_eq!(cmd, "opencode --prompt go");
        let (cmd, paste) = with_first_prompt("opencode", "opencode", Some("do the thing"));
        assert!(!paste);
        assert_eq!(cmd, "opencode --prompt 'do the thing'");
        let (cmd, paste) = with_first_prompt("claude", "claude", Some("hi"));
        assert_eq!(cmd, "claude");
        assert!(paste);
        let (cmd, paste) = with_first_prompt("opencode", "opencode", None);
        assert_eq!(cmd, "opencode");
        assert!(!paste);
    }

    #[test]
    fn opencode_prompt_escapes_quotes() {
        let (cmd, _) = with_first_prompt("opencode", "opencode", Some("it's fine"));
        assert_eq!(cmd, "opencode --prompt 'it'\\''s fine'");
    }
}
