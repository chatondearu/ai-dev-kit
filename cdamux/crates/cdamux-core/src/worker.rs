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
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
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
    #[error("worker cap reached: {live}/{cap} live workers ([orchestrator].max_workers)")]
    Cap { live: u32, cap: u32 },
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
    /// Spawn in the target repo root (no new worktree/branch); with `repo`, that root.
    pub here: bool,
    /// Target repo toplevel (from `--repo`); when set, worktree/here use this instead of caller cwd.
    pub repo: Option<&'a Path>,
    pub worktree_root: Option<&'a Path>,
    pub agents: &'a AgentsConfig,
    /// Fallback cwd when the caller's host path cannot be resolved.
    pub cwd: &'a Path,
    /// From `[orchestrator].max_workers` (`0` = no cap).
    pub max_workers: u32,
    /// From `[worktrees].min_free_gib` (`0` = no disk guard).
    pub min_free_gib: u64,
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
    /// Worktree (or `--here` checkout) path. Empty when neither tmux nor the registry has it.
    pub path: String,
}

/// One line a human can act on: tmux session name and worktree path.
pub fn worker_ls_line(row: &WorkerRow) -> String {
    if row.path.is_empty() {
        row.host.clone()
    } else {
        format!("{}\t{}", row.host, row.path)
    }
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
pub(crate) fn write_first_prompt(host: &str, prompt: &str) -> Option<PathBuf> {
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
            state_list(StateKind::Agents)
                .into_iter()
                .find_map(|(id, _, v)| {
                    (v.get("host").and_then(|h| h.as_str()) == Some(host)).then_some(id)
                })
        })
        .unwrap_or_default()
}

/// Ensure the orchestrator has a long-lived run id (mint + write when missing).
pub fn ensure_orch_run(
    caller_id: &str,
    caller: &Value,
    repo: &Path,
) -> Result<String, WorkerError> {
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
    let panes = crate::tmux::list_panes_all();
    let _ = crate::restore::reap_dead_agent_panes(&panes);
    let sessions = crate::tmux::list_tmux_sessions().unwrap_or_default();
    let live: Vec<String> = sessions.iter().map(|s| s.name.clone()).collect();
    let session_path: HashMap<String, String> =
        sessions.into_iter().map(|s| (s.name, s.path)).collect();
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
            run: v.get("run").and_then(|r| r.as_str()).map(str::to_string),
            status: v.get("status").and_then(|s| s.as_str()).map(str::to_string),
            path: worker_path(
                &v.get("host").and_then(|h| h.as_str()).unwrap_or(""),
                &session_path,
            ),
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

/// One agent removed by [`kill_worker`].
#[derive(Debug, Clone)]
pub struct KilledAgent {
    pub id: String,
    /// tmux session it ran in; empty when its pane was already gone.
    pub host: String,
    pub report: crate::state_finalize::FinalizeReport,
}

/// `cdamux worker kill`: permanent kill of a worker (by name, id or host) and, with
/// `subtree`, of every agent below it. `caller` limits the target to its own workers
/// (an orchestrator) and, per Q23, to finished ones (`done` / `idle` / `error`: the
/// maintainer kills a working one); `None` (a human outside any agent) may name any
/// agent. A name or host matching several records is refused: names are unique per
/// orchestrator only, and a ghost record can share a live host.
pub fn kill_worker(
    caller: Option<&str>,
    key: &str,
    subtree: bool,
) -> Result<Vec<KilledAgent>, WorkerError> {
    let hits: Vec<(String, Value)> = state_list(StateKind::Agents)
        .into_iter()
        .filter(|(aid, _, v)| {
            caller.is_none_or(|c| {
                v.get("role").and_then(Value::as_str) == Some("worker")
                    && v.get("parent").and_then(Value::as_str) == Some(c)
            }) && (aid == key
                || v.get("name").and_then(Value::as_str) == Some(key)
                || v.get("host").and_then(Value::as_str) == Some(key))
        })
        .map(|(aid, _, v)| (aid, v))
        .collect();
    let id = match hits.iter().find(|(aid, _)| aid == key) {
        Some((aid, _)) => aid.clone(),
        None if hits.len() == 1 => hits[0].0.clone(),
        None if hits.is_empty() => return Err(WorkerError::NotFound(key.to_string())),
        None => {
            let ids: Vec<&str> = hits.iter().map(|(aid, _)| aid.as_str()).collect();
            return Err(WorkerError::Other(format!(
                "'{key}' is ambiguous ({}); pass the agent id",
                ids.join(", ")
            )));
        }
    };
    let targets = if subtree {
        crate::state_finalize::agent_subtree(&id)
    } else {
        vec![id]
    };
    if caller.is_some() {
        for t in &targets {
            let status = state_get(StateKind::Agents, t)?
                .and_then(|v| v.get("status")?.as_str().map(str::to_string))
                .unwrap_or_default();
            if !matches!(status.as_str(), "done" | "idle" | "error") {
                return Err(WorkerError::Other(format!(
                    "refuse to kill {t}: status '{status}' (only done/idle/error; ask the maintainer)"
                )));
            }
        }
    }
    let mut out = Vec::new();
    for t in targets {
        let host = state_get(StateKind::Agents, &t)?
            .and_then(|v| v.get("host")?.as_str().map(str::to_string))
            .unwrap_or_default();
        let report = crate::state_finalize::kill_agent_permanent(&t)?;
        out.push(KilledAgent { id: t, host, report });
    }
    Ok(out)
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
    list_workers(caller_id).ok().is_some_and(|rows| {
        rows.iter()
            .any(|r| r.name == name || r.host == slug_session(name))
    })
}

/// Quiet time after which an `idle` worker gives its cap slot back (Q9: 10 min, not
/// the 10 s paste window: a worker between two relays must keep its slot).
pub const IDLE_CAP_RELEASE_SECS: u32 = 600;

/// Count workers that count toward `[orchestrator].max_workers`.
/// Live = `role=worker`, host that passes `host_alive`, status not `done`/`error`, and
/// a parent orchestrator that is still live (a recorded agent whose host passes
/// `host_alive`): workers of a dead or archived orchestrator never hold the cap. A
/// worker with no `parent` key still counts. `idle` past [`IDLE_CAP_RELEASE_SECS`]
/// with no open ask does not count (`host_idle_secs` is the pane's idle time; `None`
/// keeps the slot). Missing status still counts. Dormant hosts fail `host_alive`.
pub fn count_live_workers<'a, I>(
    agents: I,
    host_alive: &dyn Fn(&str) -> bool,
    host_idle_secs: &dyn Fn(&str) -> Option<u32>,
) -> u32
where
    I: IntoIterator<Item = &'a Value>,
{
    let agents: Vec<&Value> = agents.into_iter().collect();
    let hosts: HashMap<&str, &str> = agents
        .iter()
        .filter_map(|v| Some((v.get("id")?.as_str()?, v.get("host")?.as_str()?)))
        .filter(|(_, h)| !h.is_empty())
        .collect();
    let host_of = |id: &str| hosts.get(id).copied();
    agents
        .iter()
        .filter(|v| {
            if v.get("role").and_then(|r| r.as_str()) != Some("worker") {
                return false;
            }
            if matches!(
                v.get("status").and_then(|s| s.as_str()),
                Some("done") | Some("error")
            ) {
                return false;
            }
            let Some(host) = v
                .get("host")
                .and_then(|h| h.as_str())
                .filter(|h| !h.is_empty())
            else {
                return false;
            };
            if !host_alive(host) {
                return false;
            }
            if let Some(parent) = v.get("parent").and_then(Value::as_str).filter(|p| !p.is_empty()) {
                if !host_of(parent).is_some_and(host_alive) {
                    return false;
                }
            }
            !idle_releases_cap(v, host_idle_secs(host))
        })
        .count() as u32
}

/// Long-quiet `idle` with no open ask is finished work: it must not hold the cap.
fn idle_releases_cap(agent: &Value, idle_secs: Option<u32>) -> bool {
    if agent.get("status").and_then(|s| s.as_str()) != Some("idle") {
        return false;
    }
    if !idle_secs.is_some_and(|s| s >= IDLE_CAP_RELEASE_SECS) {
        return false;
    }
    let Some(id) = agent
        .get("id")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
    else {
        return true;
    };
    let Some(parent) = agent
        .get("parent")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
    else {
        return true;
    };
    !crate::msg::has_pending_ask(id, parent)
}

fn worker_path(host: &str, session_path: &HashMap<String, String>) -> String {
    session_path
        .get(host)
        .filter(|p| !p.is_empty())
        .cloned()
        .or_else(|| crate::session_registry::load(host).map(|r| r.cwd.display().to_string()))
        .unwrap_or_default()
}

fn checkout_branch(path: &Path) -> Option<String> {
    // `rev-parse --abbrev-ref` is `HEAD` on an unborn branch; symbolic-ref is `main`.
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    current_branch(path)
}

fn refuse_here_checkout(path: &Path) -> Result<(), WorkerError> {
    let branch = checkout_branch(path).unwrap_or_default();
    if branch == "main" || branch == "master" {
        return Err(WorkerError::Other(format!(
            "refuse --here on branch {branch}"
        )));
    }
    if git_porcelain_dirty(path) {
        return Err(WorkerError::Other("refuse --here on a dirty tree".into()));
    }
    Ok(())
}

fn git_porcelain_dirty(path: &Path) -> bool {
    let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
    else {
        return false;
    };
    out.status.success() && out.stdout.iter().any(|b| !b.is_ascii_whitespace())
}

fn refuse_if_at_cap(max_workers: u32) -> Result<(), WorkerError> {
    if max_workers == 0 {
        return Ok(());
    }
    let panes = crate::tmux::list_panes_all();
    // Free slots held by panes whose agent CLI exited before counting them (L1).
    let _ = crate::restore::reap_dead_agent_panes(&panes);
    let live_hosts: Vec<String> = crate::tmux::list_tmux_sessions()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.name)
        .collect();
    let agents: Vec<Value> = state_list(StateKind::Agents)
        .into_iter()
        .map(|(_, _, v)| v)
        .collect();
    let live = count_live_workers(
        agents.iter(),
        &|h| live_hosts.iter().any(|x| x == h),
        &|h| panes.get(h).and_then(|p| p.idle_seconds),
    );
    if live >= max_workers {
        return Err(WorkerError::Cap {
            live,
            cap: max_workers,
        });
    }
    Ok(())
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

    // Cap before any side effect (worktree / run mint / tmux).
    refuse_if_at_cap(req.max_workers)?;

    // Read before creating the worktree so a bad path leaves no orphan worktree/branch.
    let user_prompt = match req.prompt_file {
        Some(p) => Some(fs::read_to_string(p).map_err(|e| WorkerError::Io(e.to_string()))?),
        None => None,
    };
    let prompt = compose_worker_first_prompt(user_prompt.as_deref());

    // Run stays on the caller's orchestrator cwd even when `--repo` retargets the worktree.
    let caller_cwd = resolve_caller_cwd(&caller, req.cwd);
    let base_cwd = req
        .repo
        .map(Path::to_path_buf)
        .unwrap_or_else(|| caller_cwd.clone());
    if req.here {
        refuse_here_checkout(&base_cwd)?;
    }
    let worktree_root = req
        .worktree_root
        .map(Path::to_path_buf)
        .or_else(|| cdamux_state_dir("worktrees"));
    let guard_path = if req.here {
        Some(base_cwd.clone())
    } else {
        worktree_root.clone()
    };
    if let Some(p) = guard_path {
        crate::worktree_clean::check_free_space(&p, req.min_free_gib).map_err(WorkerError::Other)?;
    }
    let run = ensure_orch_run(req.caller_id, &caller, &caller_cwd)?;

    let target_repo = git_toplevel(&base_cwd);
    // Resolve the command before any side effect: an unknown agent leaves nothing behind.
    let base_cmd = resolve_agent_command(req.agents, &client)?;
    let (spawn_path, new_worktree) = if req.here {
        (base_cwd.clone(), None)
    } else {
        let root = worktree_root.ok_or(WorkerError::NoWorktreeRoot)?;
        let repo = target_repo.clone().ok_or(WorkerError::NotGit)?;
        let base_branch = current_branch(&repo).unwrap_or_else(|| "HEAD".into());
        let branch = format!("worker/{run}/{name}");
        let dest = resolve_worktree_dest(&root.join(&run), &name);
        if dest.exists() {
            return Err(WorkerError::Exists(dest.display().to_string()));
        }
        // Never delete a branch this spawn did not create (it may hold unpushed work).
        if crate::worktree::list_local_branches(&repo).contains(&branch) {
            return Err(WorkerError::Exists(format!("branch {branch}")));
        }
        if let Err(e) = worktree_add_new_branch(&repo, &dest, &branch, &base_branch) {
            // A failed or timed-out add can leave a half checkout and the branch; an
            // "already exists" means a concurrent spawn owns them, so leave them alone.
            if !e.to_string().contains("already exists") {
                crate::worktree::remove_new_worktree(&repo, &dest, &branch);
            }
            return Err(e.into());
        }
        (dest.clone(), Some((repo, dest, branch)))
    };

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
    let outcome = match spawn_tmux_session_ex(
        &host,
        &spawn_path,
        &launch_cmd,
        PaneLayout::Single,
        false,
        Some(client.as_str()),
        Some(parent_host.as_str()),
        extra,
    ) {
        Ok(o) => o,
        Err(e) => {
            // M10: a session created before the failure (send-keys, split) goes first,
            // so no pane keeps the worktree open; then no orphan worktree + branch, so
            // the same name can be retried.
            if crate::send::tmux_has_session(&host).unwrap_or(false) {
                let _ = Command::new("tmux")
                    .args(["kill-session", "-t", &format!("={host}")])
                    .status();
            }
            if let Some((repo, dest, branch)) = &new_worktree {
                crate::worktree::remove_new_worktree(repo, dest, branch);
            }
            return Err(e.into());
        }
    };

    let agent_id = resolve_spawned_agent_id(&host);

    if let Some(repo) = target_repo {
        let _ = state_append(
            StateKind::Agents,
            &agent_id,
            &json!({ "repo": repo.display().to_string() }),
        );
    }

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
    fn count_live_workers_filters_status_and_dead_hosts() {
        let agents = vec![
            json!({"role": "worker", "status": "working", "host": "live-a"}),
            json!({"role": "worker", "status": "blocked", "host": "live-b"}),
            json!({"role": "worker", "status": "idle", "host": "live-idle"}),
            // Missing status (no --cdamux-hooks) still counts when host is live.
            json!({"role": "worker", "host": "live-nostatus"}),
            json!({"role": "worker", "status": "done", "host": "live-c"}),
            json!({"role": "worker", "status": "error", "host": "live-err"}),
            // Dormant = dead host (ADR-011); dead host never counts.
            json!({"role": "worker", "status": "dormant", "host": "dead-dormant"}),
            json!({"role": "worker", "status": "working", "host": "dead"}),
            json!({"role": "orchestrator", "status": "working", "host": "live-e"}),
            json!({"role": "worker", "status": "working"}),
        ];
        let alive = |h: &str| {
            matches!(
                h,
                "live-a"
                    | "live-b"
                    | "live-idle"
                    | "live-nostatus"
                    | "live-c"
                    | "live-err"
                    | "live-e"
            )
        };
        // working + blocked + idle (idle unknown) + missing status = 4; done/error/dead/orch excluded
        assert_eq!(count_live_workers(agents.iter(), &alive, &|_| None), 4);
    }

    #[test]
    fn quiet_idle_and_done_do_not_hold_the_cap() {
        let agents = vec![
            json!({"role": "worker", "status": "idle", "host": "quiet"}),
            json!({"role": "worker", "status": "idle", "host": "fresh"}),
            json!({"role": "worker", "status": "done", "host": "finished"}),
            json!({"role": "worker", "status": "working", "host": "busy"}),
        ];
        let alive = |h: &str| matches!(h, "quiet" | "fresh" | "finished" | "busy");
        let idle = |h: &str| match h {
            "quiet" | "finished" => Some(IDLE_CAP_RELEASE_SECS),
            // Past the 10 s paste window but well inside the 10 min slot (Q9).
            "fresh" => Some(crate::msg::DEFAULT_NUDGE_QUIET_SECS * 6),
            _ => None,
        };
        // quiet idle released; fresh idle + working still count; done never counts
        assert_eq!(count_live_workers(agents.iter(), &alive, &idle), 2);
    }

    #[test]
    fn workers_of_a_dead_orchestrator_do_not_hold_the_cap() {
        let agents = vec![
            json!({"id": "a-live", "role": "orchestrator", "host": "c-live"}),
            json!({"id": "a-dead", "role": "orchestrator", "host": "c-dead"}),
            json!({"role": "worker", "status": "working", "host": "w1", "parent": "a-live"}),
            json!({"role": "worker", "status": "working", "host": "w2", "parent": "a-dead"}),
            // Parent record archived / never existed.
            json!({"role": "worker", "status": "working", "host": "w3", "parent": "a-gone"}),
        ];
        let alive = |h: &str| matches!(h, "c-live" | "w1" | "w2" | "w3");
        assert_eq!(count_live_workers(agents.iter(), &alive, &|_| None), 1);
    }

    #[test]
    fn human_kill_refuses_an_ambiguous_name() {
        let _g = crate::paths::env_lock();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", home.path());
        for id in ["a-api1", "a-api2"] {
            state_set(StateKind::Agents, id, &json!({"id": id, "role": "worker", "name": "api"})).unwrap();
        }
        let err = kill_worker(None, "api", false).unwrap_err().to_string();
        std::env::remove_var("CDAMUX_HOME");
        assert!(err.contains("ambiguous"), "{err}");
    }

    #[test]
    fn refuse_if_at_cap_zero_is_unlimited() {
        assert!(refuse_if_at_cap(0).is_ok());
    }

    #[test]
    fn cap_error_message_matches_contract() {
        let e = WorkerError::Cap { live: 6, cap: 6 };
        assert_eq!(
            e.to_string(),
            "worker cap reached: 6/6 live workers ([orchestrator].max_workers)"
        );
    }

    #[test]
    fn refuse_if_at_cap_ignores_dead_hosts() {
        let _g = crate::paths::env_lock();
        let tmux_dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("TMUX_TMPDIR", tmux_dir.path());
        std::env::set_var("CDAMUX_HOME", home.path());
        state_set(
            StateKind::Agents,
            "a-w-cap",
            &json!({"id": "a-w-cap", "role": "worker", "host": "gone", "status": "working"}),
        )
        .unwrap();
        let ok = refuse_if_at_cap(1);
        std::env::remove_var("TMUX_TMPDIR");
        std::env::remove_var("CDAMUX_HOME");
        assert!(ok.is_ok(), "dead host must not count toward the cap");
    }

    #[test]
    fn refuse_if_at_cap_errors_when_live_at_cap() {
        if std::process::Command::new("tmux")
            .arg("-V")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            eprintln!("tmux not on PATH: skipped");
            return;
        }
        let _g = crate::paths::env_lock();
        let tmux_dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("TMUX_TMPDIR", tmux_dir.path());
        std::env::set_var("CDAMUX_HOME", home.path());
        std::env::remove_var("TMUX");
        assert!(
            std::process::Command::new("tmux")
                .args(["new-session", "-d", "-s", "live-w", "sleep", "60"])
                .status()
                .unwrap()
                .success(),
            "throwaway tmux session"
        );
        state_set(
            StateKind::Agents,
            "a-w-live",
            &json!({"id": "a-w-live", "role": "worker", "host": "live-w", "status": "working"}),
        )
        .unwrap();
        let err = refuse_if_at_cap(1);
        let sock = std::process::Command::new("tmux")
            .args(["display-message", "-p", "#{socket_path}"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        if sock.contains(tmux_dir.path().to_str().unwrap_or("___")) {
            let _ = std::process::Command::new("tmux")
                .args(["kill-server"])
                .status();
        }
        std::env::remove_var("TMUX_TMPDIR");
        std::env::remove_var("CDAMUX_HOME");
        match err {
            Err(WorkerError::Cap { live: 1, cap: 1 }) => {}
            other => panic!("expected Cap {{ live: 1, cap: 1 }}, got {other:?}"),
        }
    }

    #[test]
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
        assert_eq!(worker_ls_line(&rows[0]), "gone-host");
    }

    #[test]
    fn spawn_here_on_main_is_refused() {
        let _g = crate::paths::env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["init", "-b", "main"])
                .status()
                .unwrap()
                .success(),
            "git init -b main"
        );
        std::env::set_var("TMUX_TMPDIR", tmp.path());
        std::env::set_var("CDAMUX_HOME", tmp.path().join("home"));
        std::env::remove_var("TMUX");
        state_set(
            StateKind::Agents,
            "a-orchh",
            &json!({
                "id": "a-orchh",
                "role": "orchestrator",
                "host": "c-orch-here",
                "client": "cursor"
            }),
        )
        .unwrap();
        let agents = crate::AgentsConfig::default();
        let err = spawn_worker(WorkerSpawnRequest {
            name: "w",
            caller_id: "a-orchh",
            agent: None,
            prompt_file: None,
            here: true,
            repo: Some(&repo),
            worktree_root: None,
            agents: &agents,
            cwd: &repo,
            max_workers: 0,
            min_free_gib: 0,
        });
        std::env::remove_var("TMUX_TMPDIR");
        std::env::remove_var("CDAMUX_HOME");
        match err {
            Err(WorkerError::Other(msg)) => {
                assert!(msg.contains("branch main"), "{msg}");
                assert!(!msg.contains('\n'), "{msg}");
            }
            other => panic!("expected refuse --here on main, got {other:?}"),
        }

        let dirty = tmp.path().join("dirty");
        fs::create_dir(&dirty).unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(&dirty)
            .args(["init", "-b", "feature"])
            .status()
            .unwrap()
            .success());
        fs::write(dirty.join("untracked"), "x").unwrap();
        std::env::set_var("TMUX_TMPDIR", tmp.path());
        std::env::set_var("CDAMUX_HOME", tmp.path().join("home"));
        let dirty_err = spawn_worker(WorkerSpawnRequest {
            name: "w",
            caller_id: "a-orchh",
            agent: None,
            prompt_file: None,
            here: true,
            repo: Some(&dirty),
            worktree_root: None,
            agents: &agents,
            cwd: &dirty,
            max_workers: 0,
            min_free_gib: 0,
        });
        std::env::remove_var("TMUX_TMPDIR");
        std::env::remove_var("CDAMUX_HOME");
        match dirty_err {
            Err(WorkerError::Other(msg)) => assert!(msg.contains("dirty"), "{msg}"),
            other => panic!("expected refuse --here on a dirty tree, got {other:?}"),
        }
    }

    #[test]
    fn first_prompt_read_from_file_on_the_command_line() {
        let f = Path::new("/st/prompts/w1.md");
        let (cmd, paste) = with_first_prompt("opencode", "opencode", Some("go"), Some(f));
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
