//! Interactive workers under an orchestrator (`cdamux worker spawn|ls|peek`).

use crate::client_name::normalize_client_name;
use crate::paths::cdamux_state_dir;
use crate::send::{send_to_session, SendError};
use crate::session_manage::unique_session_name;
use crate::spawn::{
    resolve_agent_command, session_name_for_path, spawn_tmux_session_ex, PaneLayout, SpawnError,
    SpawnExtra, SpawnOutcome,
};
use crate::state::{
    generate_run_id, state_append, state_get, state_list, state_set, valid_state_id, StateKind,
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
    #[error("first message paste failed: {0}")]
    Send(#[from] SendError),
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

/// Build the pane launch command. Product clients read the first prompt from
/// `prompt_file` on their command line (opencode `--prompt`, others positional);
/// the typed command stays short because a tty in canonical mode keeps only
/// ~1 KB of pending input (macOS), which truncated long pasted prompts.
/// Other clients, or no file: paste after spawn. Returns `(command, paste_prompt_after)`.
pub fn with_first_prompt(
    command: &str,
    client: &str,
    prompt: Option<&str>,
    prompt_file: Option<&Path>,
) -> (String, bool) {
    if prompt.map(str::trim).filter(|s| !s.is_empty()).is_none() {
        return (command.to_string(), false);
    }
    let arg = |file: &Path| format!("\"$(cat {})\"", shell_single_quote(&file.to_string_lossy()));
    match (normalize_client_name(client), prompt_file) {
        ("opencode", Some(f)) => (format!("{command} --prompt {}", arg(f)), false),
        ("claude" | "cursor" | "codex", Some(f)) => (format!("{command} {}", arg(f)), false),
        _ => (command.to_string(), true),
    }
}

/// Write the first prompt under `$CDAMUX_HOME/prompts/<host>.md`; `None` when no home.
fn write_first_prompt(host: &str, prompt: &str) -> Option<PathBuf> {
    let dir = cdamux_state_dir("prompts")?;
    std::fs::create_dir_all(&dir).ok()?;
    let file = dir.join(format!("{host}.md"));
    std::fs::write(&file, prompt.trim()).ok()?;
    Some(file)
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

/// Default first-message prefix for `worker spawn` (cdamux msg reporting).
pub fn worker_preamble() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../prompts/worker-preamble.md"
    ))
    .trim()
}

/// Preamble alone, or preamble + user prompt from `--prompt-file`.
pub fn compose_worker_first_prompt(user: Option<&str>) -> String {
    let preamble = worker_preamble();
    match user.map(str::trim).filter(|s| !s.is_empty()) {
        Some(u) => format!("{preamble}\n\n{u}"),
        None => preamble.to_string(),
    }
}

/// Prefer session registry; fall back to agent state docs by `host` (CreatedUnrecorded).
pub(crate) fn resolve_spawned_agent_id(host: &str) -> String {
    crate::session_registry::load(host)
        .and_then(|r| r.cdamux_id)
        .or_else(|| {
            state_list(StateKind::Agents).into_iter().find_map(|(id, _, v)| {
                (v.get("host").and_then(|h| h.as_str()) == Some(host)).then_some(id)
            })
        })
        .unwrap_or_default()
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
    for (id, _, v) in state_list(StateKind::Agents) {
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
    // Hosts that are not live tmux sessions show `dormant`, like Fleet (a killed worker
    // keeps its stored `working`).
    let live: Vec<String> = crate::tmux::list_tmux_sessions()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.name)
        .collect();
    let mut rows: Vec<WorkerRow> = state_list(StateKind::Agents)
        .into_iter()
        .filter(|(_, _, v)| {
            v.get("role").and_then(|r| r.as_str()) == Some("worker")
                && v.get("parent").and_then(|p| p.as_str()) == Some(caller_id)
        })
        .map(|(id, _, v)| WorkerRow {
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
        .map(|mut row| {
            if !live.contains(&row.host) {
                row.status = Some("dormant".into());
            }
            row
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

    // Read before creating the worktree so a bad path leaves no orphan worktree/branch.
    let user_prompt = match req.prompt_file {
        Some(p) => Some(fs::read_to_string(p).map_err(|e| WorkerError::Io(e.to_string()))?),
        None => None,
    };
    let prompt = compose_worker_first_prompt(user_prompt.as_deref());

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
        let branch = format!("worker/{run}/{name}");
        let dest = resolve_worktree_dest(&root.join(&run), &name);
        if dest.exists() {
            return Err(WorkerError::Exists(dest.display().to_string()));
        }
        worktree_add_new_branch(&repo, &dest, &branch, &base_branch)?;
        dest
    };

    let base_cmd = resolve_agent_command(req.agents, &client)?;

    let session_base = if req.here {
        session_name_for_path(&spawn_path)
    } else {
        slug_session(&name)
    };
    let host = unique_session_name(&session_base);
    let prompt_file = write_first_prompt(&host, &prompt);
    let (launch_cmd, paste_after) =
        with_first_prompt(&base_cmd, &client, Some(&prompt), prompt_file.as_deref());

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

    let agent_id = resolve_spawned_agent_id(&host);

    if paste_after && !prompt.trim().is_empty() {
        send_to_session(&host, prompt.trim(), true)?;
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
<<<<<<< HEAD
    fn first_prompt_read_from_file_on_the_command_line() {
        let f = Path::new("/st/prompts/w1.md");
        let (cmd, paste) = with_first_prompt("opencode", "opencode", Some("go"), Some(f));
=======
    fn worker_ls_shows_dormant_when_host_is_not_live() {
        let _g = crate::paths::env_lock();
        // Throwaway tmux dir: no server there, so no host is live and the real server is untouched.
        let tmux_dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("TMUX_TMPDIR", tmux_dir.path());
        std::env::set_var("CDAMUX_HOME", home.path());
        state_set(
            StateKind::Agents,
            "a-w1",
            &json!({"id": "a-w1", "role": "worker", "parent": "a-orch", "host": "gone-host", "status": "working"}),
        )
        .unwrap();
        let rows = list_workers("a-orch").unwrap();
        std::env::remove_var("TMUX_TMPDIR");
        std::env::remove_var("CDAMUX_HOME");
        assert_eq!(rows[0].status.as_deref(), Some("dormant"));
    }

    #[test]
    fn opencode_uses_prompt_flag() {
        let (cmd, paste) = with_first_prompt("opencode", "opencode", Some("go"));
>>>>>>> origin/main
        assert!(!paste);
        assert_eq!(cmd, "opencode --prompt \"$(cat /st/prompts/w1.md)\"");
        let (cmd, paste) = with_first_prompt("claude --x", "claude", Some("hi"), Some(f));
        assert!(!paste);
        assert_eq!(cmd, "claude --x \"$(cat /st/prompts/w1.md)\"");
        // No file (no CDAMUX_HOME) or an unknown client: paste after spawn.
        let (cmd, paste) = with_first_prompt("claude", "claude", Some("hi"), None);
        assert_eq!(cmd, "claude");
        assert!(paste);
        let (cmd, paste) = with_first_prompt("aider", "aider", Some("hi"), Some(f));
        assert_eq!(cmd, "aider");
        assert!(paste);
        let (cmd, paste) = with_first_prompt("opencode", "opencode", None, Some(f));
        assert_eq!(cmd, "opencode");
        assert!(!paste);
    }

    #[test]
    fn prompt_file_path_is_quoted() {
        let f = Path::new("/st/it's dir/w1.md");
        let (cmd, _) = with_first_prompt("opencode", "opencode", Some("go"), Some(f));
        assert_eq!(cmd, "opencode --prompt \"$(cat '/st/it'\\''s dir/w1.md')\"");
    }

    #[test]
    fn worker_preamble_mentions_cdamux_msg() {
        let p = worker_preamble();
        assert!(p.contains("cdamux msg"), "{p}");
        assert!(p.contains("--kind ask"), "{p}");
        let composed = compose_worker_first_prompt(Some("do the thing"));
        assert!(composed.starts_with(p));
        assert!(composed.contains("do the thing"));
        assert_eq!(compose_worker_first_prompt(None), p);
    }
}
