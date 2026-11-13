//! cdamux — CLI for agent session orchestration on tmux.

mod app_tabs;
mod fleet_draw;
mod fleet_tree;
mod graph_view;
mod keymap;
mod refresh;
mod review_tui;
mod settings_form;
mod spawn_modal;
#[cfg(test)]
mod test_render;
mod tui;
mod widgets;

use cdamux_core::{
    cdamux_home, command_on_path, config_path, configured_agent_ids, default_agent_name,
    discover_projects, ensure_home_session, ensure_note_file, format_unread_digest, git_diff_unified, git_toplevel,
    home_session_new_argv, list_tmux_sessions, list_worktrees, list_workers, load_config,
    note_refs_for_projects, current_tmux_session, kill_tmux_session, list_panes_all, msg_send,
    parents_first, peek_worker, read_messages, parse_unified_diff, resolve_agent_command,
    restore_one, restore_targets, restart_plan, session_registry, resolve_note_path, send_to_session,
    session_name_for_path, spawn_orchestrator, spawn_tmux_session_with_layout, spawn_worker,
    state_append, state_get, state_set, run_gc, tmux_has_session, unique_session_name, valid_kind,
    AttachMode, DEFAULT_NUDGE_QUIET_SECS, DiffError, GroupBy, OrchNewRequest, PaneLayout,
    SendError, SpawnError, StateKind, WorkerSpawnRequest, ORCHESTRATOR_SKILL,
};
use clap::{Parser, Subcommand};
use skim::prelude::*;
use std::io::{self, BufRead, Cursor, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

const CUSTOM_PATH_ITEM: &str = "[custom path…]";

#[derive(Parser)]
#[command(name = "cdamux", version, about = "AI agent sessions on tmux")]
struct Cli {
    /// Minimal picker: switch-client then exit (for tmux display-popup -E).
    #[arg(long, global = true)]
    pick: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Health check for tmux and config.
    Doctor,
    /// List tmux sessions (read directly from tmux).
    List {
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum, default_value_t = GroupByArg::Project)]
        group_by: GroupByArg,
    },
    /// Discover configured project roots on disk.
    Projects,
    /// Attach to a tmux session name.
    Attach {
        session: String,
    },
    /// Spawn agent session in a project directory.
    Spawn {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        path: Option<String>,
        /// Pick project path and agent interactively (skim).
        #[arg(long, short)]
        interactive: bool,
    },
    /// Recreate recorded sessions that are not live, resuming their agent conversation.
    /// An orchestrator name also restores its workers (parents first).
    #[command(group = clap::ArgGroup::new("target").required(true).args(["name", "all"]))]
    Restore {
        /// Recorded session name (see the restore registry, ADR-009).
        name: Option<String>,
        /// Every recorded session that is not live, parents before children.
        #[arg(long)]
        all: bool,
    },
    /// Kill then restore live recorded agent sessions on their conversation (e.g. after a kit update).
    #[command(group = clap::ArgGroup::new("target").required(true).args(["name", "all"]))]
    Restart {
        /// Recorded session name.
        name: Option<String>,
        /// Every live recorded agent session; the current client's session goes last.
        #[arg(long)]
        all: bool,
        /// Also restart sessions whose `@cdamux_state` says the agent is working.
        #[arg(long)]
        force: bool,
    },
    /// Send text to a session pane without attaching (tmux send-keys).
    #[command(after_help = "Examples:\n  cdamux send t1 \"echo hi\" --enter\n  cdamux send --enter t1 echo hi\n\nText starting with '-' needs `--` first: cdamux send t1 --enter -- -la")]
    Send {
        /// tmux session name (targets active window + active pane).
        session: String,
        /// Literal text to send (`-l`); words are joined with spaces.
        message: Vec<String>,
        /// Append Enter after the message.
        #[arg(long)]
        enter: bool,
    },
    /// Inspect git diff hunks and send a review prompt to an agent session.
    Review {
        /// tmux session that receives the composed prompt (send-keys + Enter).
        session: String,
        /// Repo path for `git diff` (default: tmux session_path for `session`).
        #[arg(long)]
        path: Option<String>,
        /// Lead sentence prepended to the hunk payload.
        #[arg(long, default_value = "Please review these diff hunks and suggest fixes.")]
        instruction: String,
    },
    /// Per-project markdown notes (local files).
    Notes {
        #[command(subcommand)]
        command: NotesCommands,
    },
    /// Git worktrees for a repository (explicit opt-in; not orch `.orch/worktrees/`).
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Read/write agent and run state under `CDAMUX_HOME` (ADR-011).
    State {
        #[command(subcommand)]
        command: StateCommands,
    },
    /// Inbox send/read under `$CDAMUX_HOME/inbox/` (ADR-011 §4).
    Msg {
        #[command(subcommand)]
        command: MsgCommands,
    },
    /// Interactive workers under the caller's orchestrator (ADR-011).
    Worker {
        #[command(subcommand)]
        command: WorkerCommands,
    },
    /// Long-lived orchestrator session (ADR-011).
    Orch {
        #[command(subcommand)]
        command: OrchCommands,
    },
}

#[derive(Subcommand)]
enum OrchCommands {
    /// Start a client with `role=orchestrator`, a fresh run, and skill `cda-orchestrator`.
    New {
        /// Product client (`claude`|`cursor`|`opencode`|`codex`); default: config `agents.default`.
        #[arg(long)]
        agent: Option<String>,
        /// Working directory (default: cwd).
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum WorkerCommands {
    /// Spawn a worker (worktree+branch by default; requires `$CDAMUX_AGENT_ID`).
    Spawn {
        /// Logical worker name (also used for `--to` / peek).
        name: String,
        /// Product client (`claude`|`cursor`|`opencode`|`codex`); default: caller's client.
        #[arg(long)]
        agent: Option<String>,
        /// First message (opencode: `--prompt`; others: paste after launch).
        #[arg(long)]
        prompt_file: Option<PathBuf>,
        /// Use the caller's cwd (skip worktree + branch).
        #[arg(long)]
        here: bool,
    },
    /// List workers whose `parent` is `$CDAMUX_AGENT_ID`.
    Ls,
    /// Show the last N lines of a worker pane.
    Peek {
        name: String,
        #[arg(long, default_value_t = 40)]
        lines: i32,
    },
}

#[derive(Subcommand)]
enum StateCommands {
    /// Atomically replace `agents/<id>.json` or `runs/<id>.json`.
    Set {
        /// `agents` or `runs`.
        kind: String,
        /// Entity id (`a-xxxxx`, run id, …).
        id: String,
        /// JSON document (private format).
        json: String,
    },
    /// Print the entity JSON to stdout.
    Get {
        kind: String,
        id: String,
    },
    /// Shallow-merge JSON object keys into the entity file.
    Append {
        kind: String,
        id: String,
        json: String,
    },
    /// Archive then remove done/error agents and finished runs past retention.
    Gc {
        /// Override `[state].retention_days` from config (must be ≥ 1).
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        retention_days: Option<u32>,
    },
}

#[derive(Subcommand)]
enum MsgCommands {
    /// Append a message to the target inbox (and deliver: paste worker / nudge orch).
    Send {
        /// Target agent id (`a-xxxxx`) or tmux session / host name.
        #[arg(long)]
        to: String,
        /// `report` | `ask` | `done` | `error` | `relay` | `answer`.
        #[arg(long, default_value = "report")]
        kind: String,
        /// Optional path to `worker-result.json` or a log.
        #[arg(long)]
        r#ref: Option<String>,
        /// Sender agent id (default: `$CDAMUX_AGENT_ID`).
        #[arg(long)]
        from: Option<String>,
        /// Message body (words joined with spaces).
        text: Vec<String>,
    },
    /// Print unread inbox lines for an agent; `--ack` advances the cursor.
    Read {
        /// Agent id (default: `$CDAMUX_AGENT_ID`).
        #[arg(long)]
        agent: Option<String>,
        /// Advance the ack cursor past the lines printed.
        #[arg(long)]
        ack: bool,
        /// Print a short digest instead of raw JSONL.
        #[arg(long)]
        digest: bool,
    },
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// List linked worktrees (`git worktree list`).
    List {
        /// Repository path (default: current directory).
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(Subcommand)]
enum NotesCommands {
    /// List note paths for configured projects (and tmux session cwds).
    List,
    /// Print a note (plain or rendered markdown).
    Show {
        #[arg(long, conflicts_with = "session")]
        path: Option<String>,
        #[arg(long, conflicts_with = "path")]
        session: Option<String>,
        /// Render markdown in the terminal (pulldown-cmark).
        #[arg(long)]
        render: bool,
    },
    /// Open note in $EDITOR (or config notes.editor).
    Edit {
        #[arg(long, conflicts_with = "session")]
        path: Option<String>,
        #[arg(long, conflicts_with = "path")]
        session: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum GroupByArg {
    Project,
    Cwd,
    Agent,
    GroupPath,
    None,
}

impl From<GroupByArg> for GroupBy {
    fn from(v: GroupByArg) -> Self {
        match v {
            GroupByArg::Project => GroupBy::Project,
            GroupByArg::Cwd => GroupBy::Cwd,
            GroupByArg::Agent => GroupBy::Agent,
            GroupByArg::GroupPath => GroupBy::GroupPath,
            GroupByArg::None => GroupBy::None,
        }
    }
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    if cli.pick {
        return cmd_pick();
    }

    match cli.command {
        // Outside tmux: enter the home session (starts server → continuum can restore).
        None if std::env::var_os("TMUX").is_none() => enter_home_session(),
        None => tui::run_dashboard(),
        Some(Commands::Doctor) => cmd_doctor(),
        Some(Commands::List { json, group_by }) => cmd_list(json, group_by.into()),
        Some(Commands::Projects) => cmd_projects(),
        Some(Commands::Attach { session }) => cmd_attach(&session),
        Some(Commands::Spawn {
            agent,
            path,
            interactive,
        }) => cmd_spawn(agent, path, interactive),
        Some(Commands::Restore { name, all }) => cmd_restore(name, all),
        Some(Commands::Restart { name, all, force }) => cmd_restart(name, all, force),
        Some(Commands::Send {
            session,
            message,
            enter,
        }) => cmd_send(&session, &message, enter),
        Some(Commands::Review {
            session,
            path,
            instruction,
        }) => cmd_review(&session, path, &instruction),
        Some(Commands::Notes { command }) => match command {
            NotesCommands::List => cmd_notes_list(),
            NotesCommands::Show {
                path,
                session,
                render,
            } => cmd_notes_show(path, session, render),
            NotesCommands::Edit { path, session } => cmd_notes_edit(path, session),
        },
        Some(Commands::Worktree { command }) => match command {
            WorktreeCommands::List { path } => cmd_worktree_list(path),
        },
        Some(Commands::State { command }) => match command {
            StateCommands::Set { kind, id, json } => cmd_state_set(&kind, &id, &json),
            StateCommands::Get { kind, id } => cmd_state_get(&kind, &id),
            StateCommands::Append { kind, id, json } => cmd_state_append(&kind, &id, &json),
            StateCommands::Gc { retention_days } => cmd_state_gc(retention_days),
        },
        Some(Commands::Msg { command }) => match command {
            MsgCommands::Send {
                to,
                kind,
                r#ref,
                from,
                text,
            } => cmd_msg_send(&to, &kind, r#ref.as_deref(), from.as_deref(), &text),
            MsgCommands::Read { agent, ack, digest } => {
                cmd_msg_read(agent.as_deref(), ack, digest)
            }
        },
        Some(Commands::Worker { command }) => match command {
            WorkerCommands::Spawn {
                name,
                agent,
                prompt_file,
                here,
            } => cmd_worker_spawn(&name, agent.as_deref(), prompt_file.as_deref(), here),
            WorkerCommands::Ls => cmd_worker_ls(),
            WorkerCommands::Peek { name, lines } => cmd_worker_peek(&name, lines),
        },
        Some(Commands::Orch { command }) => match command {
            OrchCommands::New { agent, path } => {
                cmd_orch_new(agent.as_deref(), path.as_deref())
            }
        },
    }
}

fn cmd_orch_new(agent: Option<&str>, path: Option<&std::path::Path>) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("orch new: {e}");
            return ExitCode::from(1);
        }
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = path.unwrap_or(&cwd);
    let req = OrchNewRequest {
        path,
        agent,
        agents: &cfg.agents,
        switch_to_session: std::env::var_os("TMUX").is_some(),
    };
    match spawn_orchestrator(req) {
        Ok(r) => {
            println!(
                "spawned orchestrator id={} host={} run={} client={} skill={} path={}{}",
                r.agent_id,
                r.host,
                r.run,
                r.client,
                ORCHESTRATOR_SKILL,
                r.path.display(),
                r.outcome.note()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("orch new: {e}");
            ExitCode::from(1)
        }
    }
}

fn resolve_caller_agent_id() -> Result<String, ExitCode> {
    match std::env::var("CDAMUX_AGENT_ID") {
        Ok(id) if !id.trim().is_empty() => Ok(id.trim().to_string()),
        _ => {
            eprintln!("worker: must run inside an orchestrator (set CDAMUX_AGENT_ID)");
            Err(ExitCode::from(2))
        }
    }
}

fn cmd_worker_spawn(
    name: &str,
    agent: Option<&str>,
    prompt_file: Option<&std::path::Path>,
    here: bool,
) -> ExitCode {
    let caller_id = match resolve_caller_agent_id() {
        Ok(id) => id,
        Err(c) => return c,
    };
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("worker spawn: {e}");
            return ExitCode::from(1);
        }
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let req = WorkerSpawnRequest {
        name,
        caller_id: &caller_id,
        agent,
        prompt_file,
        here,
        worktree_root: cfg.worktrees.root.as_deref(),
        agents: &cfg.agents,
        cwd: &cwd,
    };
    match spawn_worker(req) {
        Ok(r) => {
            println!(
                "spawned worker '{}' id={} host={} run={} path={}{}",
                r.name,
                r.agent_id,
                r.host,
                r.run,
                r.path.display(),
                r.outcome.note()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worker spawn: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_worker_ls() -> ExitCode {
    let caller_id = match resolve_caller_agent_id() {
        Ok(id) => id,
        Err(c) => return c,
    };
    match list_workers(&caller_id) {
        Ok(rows) => {
            for r in rows {
                let status = r.status.as_deref().unwrap_or("-");
                let run = r.run.as_deref().unwrap_or("-");
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    r.name, r.id, r.host, r.client, run, status
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worker ls: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_worker_peek(name: &str, lines: i32) -> ExitCode {
    let caller_id = match resolve_caller_agent_id() {
        Ok(id) => id,
        Err(c) => return c,
    };
    match peek_worker(&caller_id, name, lines) {
        Ok(text) => {
            print!("{text}");
            if !text.ends_with('\n') {
                println!();
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worker peek: {e}");
            ExitCode::from(1)
        }
    }
}

fn parse_state_kind(kind: &str) -> Result<StateKind, ExitCode> {
    StateKind::parse(kind).ok_or_else(|| {
        eprintln!("state: kind must be 'agents' or 'runs' (got '{kind}')");
        ExitCode::from(2)
    })
}

fn resolve_self_agent_id(explicit: Option<&str>) -> Result<String, ExitCode> {
    if let Some(id) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(id.to_string());
    }
    match std::env::var("CDAMUX_AGENT_ID") {
        Ok(id) if !id.trim().is_empty() => Ok(id.trim().to_string()),
        _ => {
            eprintln!("msg: set --from/--agent or CDAMUX_AGENT_ID");
            Err(ExitCode::from(2))
        }
    }
}

fn cmd_msg_send(
    to: &str,
    kind: &str,
    r#ref: Option<&str>,
    from: Option<&str>,
    text: &[String],
) -> ExitCode {
    if !valid_kind(kind) {
        eprintln!("msg send: kind must be report|ask|done|error|relay|answer");
        return ExitCode::from(2);
    }
    let body = text.join(" ");
    if body.trim().is_empty() {
        eprintln!("msg send: text required");
        return ExitCode::from(2);
    }
    let from = match resolve_self_agent_id(from) {
        Ok(id) => id,
        Err(c) => return c,
    };
    match msg_send(
        &from,
        to,
        kind,
        &body,
        r#ref,
        DEFAULT_NUDGE_QUIET_SECS,
        None,
    ) {
        Ok(out) => {
            println!("{}", out.message.id);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("msg send: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_msg_read(agent: Option<&str>, ack: bool, digest: bool) -> ExitCode {
    let id = match resolve_self_agent_id(agent) {
        Ok(id) => id,
        Err(c) => return c,
    };
    match read_messages(&id, ack) {
        Ok(msgs) => {
            if digest {
                let d = format_unread_digest(&msgs);
                if !d.is_empty() {
                    println!("{d}");
                }
            } else {
                for m in &msgs {
                    match serde_json::to_string(m) {
                        Ok(line) => println!("{line}"),
                        Err(e) => {
                            eprintln!("msg read: {e}");
                            return ExitCode::from(1);
                        }
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("msg read: {e}");
            ExitCode::from(1)
        }
    }
}

fn parse_state_json(json: &str) -> Result<serde_json::Value, ExitCode> {
    serde_json::from_str(json).map_err(|e| {
        eprintln!("state: invalid JSON: {e}");
        ExitCode::from(2)
    })
}

fn cmd_state_set(kind: &str, id: &str, json: &str) -> ExitCode {
    let kind = match parse_state_kind(kind) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let value = match parse_state_json(json) {
        Ok(v) => v,
        Err(c) => return c,
    };
    match state_set(kind, id, &value) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("state set: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_state_get(kind: &str, id: &str) -> ExitCode {
    let kind = match parse_state_kind(kind) {
        Ok(k) => k,
        Err(c) => return c,
    };
    match state_get(kind, id) {
        Ok(Some(v)) => {
            match serde_json::to_string_pretty(&v) {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("state get: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Ok(None) => {
            eprintln!("state get: not found");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("state get: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_state_append(kind: &str, id: &str, json: &str) -> ExitCode {
    let kind = match parse_state_kind(kind) {
        Ok(k) => k,
        Err(c) => return c,
    };
    let value = match parse_state_json(json) {
        Ok(v) => v,
        Err(c) => return c,
    };
    match state_append(kind, id, &value) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("state append: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_state_gc(retention_days: Option<u32>) -> ExitCode {
    let days = retention_days.unwrap_or_else(|| {
        load_config()
            .map(|c| c.state.retention_days)
            .unwrap_or(cdamux_core::DEFAULT_RETENTION_DAYS)
    });
    // Fail closed: refuse destructive GC when tmux cannot be probed.
    if let Err(e) = tmux_has_session("__cdamux_gc_probe__") {
        eprintln!("state gc: cannot probe tmux sessions ({e}); aborting");
        return ExitCode::from(1);
    }
    let host_alive = |host: &str| match tmux_has_session(host) {
        Ok(alive) => alive,
        // Rare after preflight; treat as alive so we never archive on probe failure.
        Err(_) => true,
    };
    match run_gc(days, std::time::SystemTime::now(), &host_alive) {
        Ok(report) => {
            println!(
                "state gc: archived {} agent(s), {} run(s); kept {} dormant resumable",
                report.archived_agents.len(),
                report.archived_runs.len(),
                report.skipped_dormant.len()
            );
            for id in &report.archived_agents {
                println!("  agent {id}");
            }
            for id in &report.archived_runs {
                println!("  run {id}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("state gc: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_worktree_list(path: Option<String>) -> ExitCode {
    let cwd = path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let repo = git_toplevel(&cwd).unwrap_or(cwd);
    match list_worktrees(&repo) {
        Ok(entries) => {
            for e in entries {
                let br = if e.branch.is_empty() {
                    "(detached)"
                } else {
                    e.branch.as_str()
                };
                println!("{}\t{}", e.path.display(), br);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worktree list: {e}");
            ExitCode::from(1)
        }
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn cmd_doctor() -> ExitCode {
    let tmux = Command::new("tmux").arg("-V").output();
    match tmux {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout);
            println!("tmux: {}", ver.trim());
        }
        _ => {
            eprintln!("tmux: not found on PATH");
            return ExitCode::from(1);
        }
    }

    match cdamux_home() {
        Some(p) => println!("CDAMUX_HOME: {}", p.display()),
        None => eprintln!("CDAMUX_HOME: cannot resolve (set CDAMUX_HOME, XDG_STATE_HOME, or HOME)"),
    }
    match config_path() {
        Some(p) => println!("config: {}", p.display()),
        None => eprintln!("config: cannot resolve (set CDAMUX_CONFIG, XDG_CONFIG_HOME, or HOME)"),
    }

    let mut doctor_ok = true;
    if let Ok(cfg) = load_config() {
        for line in doctor_agent_lines(&cfg) {
            doctor_ok &= !line.contains("MISSING on PATH");
            println!("{line}");
        }
    }

    match Command::new("tailscale").arg("version").output() {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout);
            println!("tailscale: {}", ver.lines().next().unwrap_or("ok"));
        }
        _ => println!("tailscale: not installed (optional)"),
    }

    if !doctor_ok {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// Lines for the dashboard Agents tab (mirrors `cdamux doctor` agent section).
pub(crate) fn doctor_agent_lines(cfg: &cdamux_core::Config) -> Vec<String> {
    let mut lines = vec![
        format!("roots: {}", cfg.projects.roots.len()),
        format!("group_by: {:?}", cfg.sessions.group_by),
        format!(
            "agents (configured): {}",
            configured_agent_ids(&cfg.agents).join(", ")
        ),
    ];
    for id in ["cursor", "opencode", "claude", "pi"] {
        let configured = agent_configured(&cfg.agents, id);
        let exe = agent_executable_name(&cfg.agents, id);
        let on_path = command_on_path(&exe);
        let tag = if configured { "configured" } else { "optional" };
        let line = if configured && !on_path {
            format!("agent {id}: MISSING on PATH ({exe}) [{tag}]")
        } else if on_path {
            format!("agent {id}: ok ({exe}) [{tag}]")
        } else {
            format!("agent {id}: not on PATH ({exe}) [{tag}]")
        };
        lines.push(line);
    }
    lines
}

fn agent_configured(agents: &cdamux_core::AgentsConfig, id: &str) -> bool {
    match id {
        "cursor" => agents.cursor.is_some(),
        "opencode" => agents.opencode.is_some(),
        "claude" => agents.claude.is_some(),
        "pi" => agents.pi.is_some(),
        _ => false,
    }
}

fn agent_executable_name(agents: &cdamux_core::AgentsConfig, id: &str) -> String {
    resolve_agent_command(agents, id)
        .unwrap_or_else(|_| id.to_string())
}


fn cmd_list(json: bool, group_by: GroupBy) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let grouped = cdamux_core::grouped_for_list_ex(
        group_by,
        &cfg.projects,
        cfg.daemon.idle_seconds_for_needs_input,
        cfg.daemon.working_warn_seconds,
    );

    if json {
        if let Err(e) = writeln!(io::stdout(), "{}", serde_json::to_string(&grouped).unwrap()) {
            eprintln!("list: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    if grouped.groups.is_empty()
        || grouped.groups.iter().all(|g| g.sessions.is_empty())
    {
        println!("(no tmux sessions)");
        return ExitCode::SUCCESS;
    }

    for group in &grouped.groups {
        if group_by != GroupBy::None {
            println!("## {}", group.key);
        }
        for s in &group.sessions {
            let st = if s.working_stale {
                format!("{} ⚠", format_state(s.state))
            } else {
                format_state(s.state).to_string()
            };
            println!("{}\t{}\t{}", s.name, s.path, st);
        }
    }
    ExitCode::SUCCESS
}

fn format_state(state: cdamux_core::SessionState) -> &'static str {
    use cdamux_core::SessionState;
    match state {
        SessionState::Idle => "idle",
        SessionState::Working => "working",
        SessionState::NeedsInput => "needs_input",
        SessionState::Blocked => "blocked",
        SessionState::Done => "done",
        SessionState::Error => "error",
        SessionState::Dormant => "dormant",
    }
}

/// Replace this process with `tmux new-session -A -s cdamux cdamux`.
fn enter_home_session() -> ExitCode {
    let _ = ensure_home_session();
    let err = Command::new("tmux").args(home_session_new_argv()).exec();
    eprintln!("cdamux: failed to enter home session: {err}");
    ExitCode::from(1)
}

fn cmd_pick() -> ExitCode {
    if std::env::var_os("TMUX").is_none() {
        eprintln!("cdamux --pick: must run inside tmux (e.g. display-popup -E)");
        return ExitCode::from(1);
    }

    let sessions = match list_tmux_sessions() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tmux: {e}");
            return ExitCode::from(1);
        }
    };

    if sessions.is_empty() {
        eprintln!("cdamux --pick: no tmux sessions");
        return ExitCode::from(1);
    }

    let lines = sessions
        .iter()
        .map(|s| format!("{}\t{}", s.name, s.path))
        .collect::<Vec<_>>()
        .join("\n");
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(lines));

    let options = SkimOptionsBuilder::default()
        .prompt("session> ".into())
        .height("100%".into())
        .build()
        .unwrap();

    let selected = match Skim::run_with(&options, Some(items)) {
        Some(out) => out.selected_items,
        None => return ExitCode::from(1),
    };

    if selected.is_empty() {
        return ExitCode::from(1);
    }

    let line = selected[0].output();
    let session = line.split('\t').next().unwrap_or(&line);

    let status = Command::new("tmux")
        .args(["switch-client", "-t", session])
        .status();

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("switch-client: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_projects() -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let projects = discover_projects(&cfg.projects);
    for p in projects {
        println!("{}\t{}\t{}", p.root_label, p.name, p.path.display());
    }
    ExitCode::SUCCESS
}

pub(crate) const DASHBOARD_REVIEW_INSTRUCTION: &str =
    "Please review these diff hunks and suggest fixes.";

/// Max foam paths painted around the selection (full list stays in `entries`).
const FOAM_SIDECAR_LIST_ROWS: usize = 6;

/// Window `[start, end)` around `sel`, biased high so under-item preview stays on screen.
fn foam_list_window(sel: usize, len: usize, max_rows: usize) -> (usize, usize) {
    if len == 0 || max_rows == 0 {
        return (0, 0);
    }
    if len <= max_rows {
        return (0, len);
    }
    // One row above the selection when possible; preview is painted under the ▸ line.
    let before = 1.min(max_rows.saturating_sub(1));
    let mut start = sel.saturating_sub(before);
    if start + max_rows > len {
        start = len - max_rows;
    }
    (start, start + max_rows)
}

/// Sidecar Foam tab: list `foam/**/*.md` for the session cwd (read-only, ADR-012).
///
/// Returns display text and a clamped selection index into the listing.
pub(crate) fn foam_sidecar_for_cwd(cwd: &str, selected: usize) -> (String, usize, Vec<String>) {
    use cdamux_core::{list_foam, FoamListing, FOAM_LIST_CAP};

    if cwd.is_empty() {
        return (
            "Select a session with a working directory.".to_string(),
            0,
            Vec::new(),
        );
    }
    let listing = list_foam(std::path::Path::new(cwd));
    match listing {
        FoamListing::Missing { project } => (
            format!(
                "No foam/ under {}\n\nCreate a foam/ tree (ADRs, MOC) to browse it here.\nCLI notes remain: `cdamux notes edit`.",
                project.display()
            ),
            0,
            Vec::new(),
        ),
        FoamListing::Empty { project, .. } => (
            format!(
                "foam/ is empty under {}\n\nAdd markdown under foam/ to list it here.",
                project.display()
            ),
            0,
            Vec::new(),
        ),
        FoamListing::Entries {
            project,
            paths,
            truncated,
            ..
        } => {
            let sel = selected.min(paths.len().saturating_sub(1));
            let (start, end) = foam_list_window(sel, paths.len(), FOAM_SIDECAR_LIST_ROWS);
            let mut lines = vec![
                format!("foam/ · {}", project.display()),
                String::new(),
            ];
            if start > 0 {
                lines.push(format!("… ({start} more above)"));
            }
            for (i, path) in paths.iter().enumerate().take(end).skip(start) {
                let mark = if i == sel { "▸" } else { " " };
                lines.push(format!("{mark} {path}"));
                // Preview under the selected row so a long list cannot push it off-screen.
                if i == sel {
                    lines.push("── preview ──".to_string());
                    lines.push(cdamux_core::preview_foam_markdown(&project, path));
                }
            }
            if end < paths.len() {
                lines.push(format!("… ({} more below)", paths.len() - end));
            }
            if truncated {
                lines.push(String::new());
                lines.push(format!("… truncated at {FOAM_LIST_CAP} files"));
            }
            (lines.join("\n"), sel, paths)
        }
    }
}

/// Result of opening a foam path in the Fleet Foam tab (#269).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FoamOpen {
    /// Non-foam path — leave the current tab alone.
    Ignored,
    /// Select an existing listing row and switch to Foam.
    Focus {
        text: String,
        selected: usize,
        entries: Vec<String>,
        path: String,
    },
    /// Foam path normalized but missing from the tree — empty/error state.
    /// `entries` kept so the operator can `[`/`]` back into the list.
    Missing {
        text: String,
        path: String,
        entries: Vec<String>,
    },
}

/// In-app Foam deep-link: normalize `raw`, select the row or show a missing state.
pub(crate) fn open_foam_in_sidecar(cwd: &str, raw: &str) -> FoamOpen {
    use cdamux_core::{find_foam_entry, normalize_foam_path, resolve_project_root};
    use std::path::Path;

    if cwd.is_empty() {
        return FoamOpen::Ignored;
    }
    let project = resolve_project_root(Path::new(cwd));
    let Some(path) = normalize_foam_path(raw, &project) else {
        return FoamOpen::Ignored;
    };
    // One listing pass at idx 0; re-render only when focusing a different row.
    let (list_text, sel0, entries) = foam_sidecar_for_cwd(cwd, 0);
    if let Some(idx) = find_foam_entry(&entries, &path) {
        let (text, selected, entries) = if idx == 0 {
            (list_text, sel0, entries)
        } else {
            foam_sidecar_for_cwd(cwd, idx)
        };
        return FoamOpen::Focus {
            text,
            selected,
            entries,
            path,
        };
    }
    // Explicit miss banner above the listing so `[`/`]` can recover browse
    // without changing session (clear = obvious empty/error, not wipe the tree).
    let miss = if entries.is_empty() && list_text.contains("No foam/") {
        format!(
            "{path} not found under {}\n\nDeep-link target missing from foam/.\n{}",
            project.display(),
            list_text
        )
    } else {
        format!(
            "{path} not found under {}\n\nDeep-link target missing from foam/.\n`[`/`]` resumes the list.\n\n{}",
            project.display(),
            list_text
        )
    };
    FoamOpen::Missing {
        text: miss,
        path,
        entries,
    }
}

/// Apply a preview-capture deep-link once per distinct foam path.
///
/// Context source: pane preview text (last `foam/...` token). Returns true when
/// Foam tab state changed. Callers should invoke only while the Foam tab is
/// visible (#294) — Preview-first open must not walk foam for deep-link.
pub(crate) fn apply_foam_deep_link_from_preview(
    cwd: &str,
    preview: &str,
    applied: &mut Option<String>,
    foam_text: &mut String,
    foam_selected: &mut usize,
    foam_entries: &mut Vec<String>,
) -> bool {
    use cdamux_core::extract_foam_path_from_text;

    let Some(raw) = extract_foam_path_from_text(preview) else {
        return false;
    };
    match open_foam_in_sidecar(cwd, &raw) {
        FoamOpen::Ignored => false,
        FoamOpen::Focus {
            text,
            selected,
            entries,
            path,
        } => {
            if applied.as_deref() == Some(path.as_str()) {
                return false;
            }
            *applied = Some(path);
            *foam_text = text;
            *foam_selected = selected;
            *foam_entries = entries;
            true
        }
        FoamOpen::Missing {
            text,
            path,
            entries,
        } => {
            if applied.as_deref() == Some(path.as_str()) {
                return false;
            }
            *applied = Some(path);
            *foam_text = text;
            *foam_selected = 0;
            *foam_entries = entries;
            true
        }
    }
}

#[cfg(test)]
mod foam_sidecar_tests {
    use super::{
        apply_foam_deep_link_from_preview, foam_list_window, foam_sidecar_for_cwd,
        open_foam_in_sidecar, FoamOpen, FOAM_SIDECAR_LIST_ROWS,
    };
    use crate::test_render::render_fleet_foam;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn lists_fixture_foam_with_selection_mark() {
        let tmp = tempdir().unwrap();
        let foam = tmp.path().join("foam");
        fs::create_dir_all(&foam).unwrap();
        fs::write(foam.join("index.md"), "# moc").unwrap();
        fs::write(foam.join("adr.md"), "a").unwrap();
        // Sorted: adr.md then index.md — selection 1 is index.md.
        let (text, sel, entries) =
            foam_sidecar_for_cwd(tmp.path().to_str().unwrap(), 1);
        assert_eq!(sel, 1);
        assert_eq!(entries.len(), 2);
        assert!(text.contains("▸ foam/index.md"), "{text}");
        assert!(text.contains("foam/adr.md"), "{text}");
        assert!(text.contains("── preview ──"), "{text}");
        assert!(text.contains("# moc"), "{text}");
    }

    #[test]
    fn missing_foam_empty_state() {
        let tmp = tempdir().unwrap();
        let (text, sel, entries) =
            foam_sidecar_for_cwd(tmp.path().to_str().unwrap(), 0);
        assert_eq!(sel, 0);
        assert!(entries.is_empty());
        assert!(text.contains("No foam/"));
    }

    #[test]
    fn selected_file_preview_shows_rendered_heading() {
        let tmp = tempdir().unwrap();
        let foam = tmp.path().join("foam");
        fs::create_dir_all(&foam).unwrap();
        fs::write(foam.join("doc.md"), "# Preview me\n\n- item\n").unwrap();
        let (text, _, _) = foam_sidecar_for_cwd(tmp.path().to_str().unwrap(), 0);
        assert!(text.contains("── preview ──"), "{text}");
        assert!(text.contains("# Preview me"), "{text}");
        assert!(text.contains("- item"), "{text}");
    }
    #[test]
    fn deep_link_focuses_matching_row() {
        let tmp = tempdir().unwrap();
        let foam = tmp.path().join("foam");
        fs::create_dir_all(foam.join("decisions")).unwrap();
        fs::write(foam.join("index.md"), "# moc").unwrap();
        fs::write(foam.join("decisions/ADR-001.md"), "a").unwrap();
        let cwd = tmp.path().to_str().unwrap();
        match open_foam_in_sidecar(cwd, "foam/decisions/ADR-001.md") {
            FoamOpen::Focus {
                selected,
                entries,
                path,
                text,
            } => {
                assert_eq!(path, "foam/decisions/ADR-001.md");
                assert_eq!(entries[selected], "foam/decisions/ADR-001.md");
                assert!(text.contains("▸ foam/decisions/ADR-001.md"));
            }
            other => panic!("expected Focus, got {other:?}"),
        }
    }

    #[test]
    fn deep_link_missing_file_empty_state() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("foam")).unwrap();
        fs::write(tmp.path().join("foam/index.md"), "x").unwrap();
        let cwd = tmp.path().to_str().unwrap();
        match open_foam_in_sidecar(cwd, "foam/nope.md") {
            FoamOpen::Missing {
                text,
                path,
                entries,
            } => {
                assert_eq!(path, "foam/nope.md");
                assert!(text.contains("not found"), "{text}");
                assert!(text.contains("foam/index.md"), "must keep listing: {text}");
                assert!(text.contains("`[`/`]` resumes"), "{text}");
                assert_eq!(entries, vec!["foam/index.md".to_string()]);
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn deep_link_ignores_non_foam() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("foam")).unwrap();
        assert_eq!(
            open_foam_in_sidecar(tmp.path().to_str().unwrap(), "prd/PRD-1.md"),
            FoamOpen::Ignored
        );
    }

    #[test]
    fn preview_deep_link_selects_once() {
        let tmp = tempdir().unwrap();
        let foam = tmp.path().join("foam");
        fs::create_dir_all(&foam).unwrap();
        fs::write(foam.join("index.md"), "# moc").unwrap();
        fs::write(foam.join("adr.md"), "a").unwrap();
        let cwd = tmp.path().to_str().unwrap();
        let mut applied = None;
        let mut text = String::new();
        let mut sel = 0usize;
        let mut entries = Vec::new();
        let preview = "working on foam/adr.md now";
        assert!(apply_foam_deep_link_from_preview(
            cwd,
            preview,
            &mut applied,
            &mut text,
            &mut sel,
            &mut entries
        ));
        assert_eq!(applied.as_deref(), Some("foam/adr.md"));
        assert!(text.contains("▸ foam/adr.md"));
        assert!(!apply_foam_deep_link_from_preview(
            cwd,
            preview,
            &mut applied,
            &mut text,
            &mut sel,
            &mut entries
        ));
    }

    #[test]
    fn deep_link_missing_resume_refreshes_listing() {
        let tmp = tempdir().unwrap();
        let foam = tmp.path().join("foam");
        fs::create_dir_all(&foam).unwrap();
        fs::write(foam.join("only.md"), "# one").unwrap();
        let cwd = tmp.path().to_str().unwrap();
        let FoamOpen::Missing { text, entries, .. } =
            open_foam_in_sidecar(cwd, "foam/gone.md")
        else {
            panic!("expected Missing");
        };
        assert!(text.contains("not found"), "{text}");
        assert!(text.contains("foam/only.md"), "banner+list: {text}");
        assert_eq!(entries, vec!["foam/only.md".to_string()]);
        // Operator presses [/] with one row: index unchanged; refresh must restore list.
        let (restored, sel, entries2) = foam_sidecar_for_cwd(cwd, 0);
        assert_eq!(sel, 0);
        assert_eq!(entries2, entries);
        assert!(restored.contains("▸ foam/only.md"), "{restored}");
        assert!(!restored.contains("not found"), "{restored}");
    }

    #[test]
    fn list_window_keeps_selection_and_biases_high() {
        assert_eq!(foam_list_window(0, 3, 10), (0, 3));
        assert_eq!(foam_list_window(35, 40, 6), (34, 40));
        let (start, end) = foam_list_window(20, 80, 6);
        assert_eq!(end - start, 6);
        assert!(start <= 20 && 20 < end);
        assert_eq!(20 - start, 1);
    }

    #[test]
    fn long_list_keeps_preview_visible_in_short_pane() {
        let tmp = tempdir().unwrap();
        let foam = tmp.path().join("foam");
        fs::create_dir_all(&foam).unwrap();
        for i in 0..40 {
            fs::write(
                foam.join(format!("f{i:02}.md")),
                format!("# File {i}\n\nbody {i}\n"),
            )
            .unwrap();
        }
        let sel = 35;
        let (text, got_sel, entries) =
            foam_sidecar_for_cwd(tmp.path().to_str().unwrap(), sel);
        assert_eq!(got_sel, sel);
        assert!(entries.len() > FOAM_SIDECAR_LIST_ROWS);
        assert!(entries.len() > 24, "fixture must exceed a 24-row pane");
        let preview_at = text
            .lines()
            .position(|l| l.contains("── preview ──"))
            .expect("preview marker");
        // Header + ellipsis + ≤10 list rows + gaps → preview well above a 24-row body.
        assert!(
            preview_at < 20,
            "preview too far down for a short pane: at line {preview_at}\n{text}"
        );
        assert!(text.contains("▸ foam/f35.md"), "{text}");
        assert!(text.contains("# File 35"), "{text}");

        let screen = render_fleet_foam(80, 24, &text).join("\n");
        assert!(screen.contains("── preview ──"), "{screen}");
        assert!(screen.contains("# File 35"), "{screen}");
        assert!(screen.contains("▸ foam/f35.md"), "{screen}");
    }
}

pub(crate) fn run_review_for_session(session: &str) -> ExitCode {
    cmd_review(session, None, DASHBOARD_REVIEW_INSTRUCTION)
}

fn cmd_review(session: &str, path: Option<String>, instruction: &str) -> ExitCode {
    let repo = match resolve_review_repo(session, path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("review: {e}");
            return ExitCode::from(1);
        }
    };

    let raw = match git_diff_unified(&repo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("review: {e}");
            return ExitCode::from(1);
        }
    };
    let hunks = parse_unified_diff(&raw);
    review_tui::run_review_tui(session, instruction, &hunks)
}

fn resolve_review_repo(session: &str, path: Option<String>) -> Result<PathBuf, DiffError> {
    if let Some(p) = path {
        return Ok(PathBuf::from(p));
    }
    let sessions = list_tmux_sessions().map_err(|_| DiffError::SessionNotFound(session.to_string()))?;
    let found = sessions.iter().find(|s| s.name == session);
    match found {
        Some(s) if !s.path.is_empty() => Ok(PathBuf::from(&s.path)),
        Some(_) => Err(DiffError::SessionPathMissing(session.to_string())),
        None => Err(DiffError::SessionNotFound(session.to_string())),
    }
}

fn cmd_send(session: &str, message: &[String], enter: bool) -> ExitCode {
    let text = message.join(" ");
    if text.is_empty() && !enter {
        eprintln!("send: provide a message and/or --enter");
        return ExitCode::from(2);
    }
    match send_to_session(session, &text, enter) {
        Ok(()) => ExitCode::SUCCESS,
        Err(SendError::SessionNotFound(name)) => {
            eprintln!("send: no tmux session named '{name}'");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("send: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_attach(session: &str) -> ExitCode {
    tmux_attach_session_with_mode(session, None)
}

pub(crate) fn tmux_attach_session_with_mode(
    session: &str,
    attach_mode: Option<AttachMode>,
) -> ExitCode {
    let attach_mode = attach_mode.unwrap_or_else(|| {
        load_config()
            .map(|c| c.sessions.attach_mode)
            .unwrap_or(AttachMode::Mirror)
    });
    let nested = std::env::var_os("TMUX").is_some();
    if nested {
        return match cdamux_core::switch_client_to_session(session) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("attach: {e}");
                ExitCode::from(1)
            }
        };
    }
    let status = {
        let mut cmd = Command::new("tmux");
        cmd.arg("attach-session");
        if attach_mode == AttachMode::Exclusive {
            cmd.arg("-d");
        }
        cmd.args(["-t", session])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    };

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("attach: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_restore(name: Option<String>, all: bool) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let records = if all {
        parents_first(session_registry::load_all())
    } else {
        let name = name.unwrap_or_default();
        match restore_targets(&name) {
            Some(r) => r,
            None => {
                eprintln!("restore: no record for session '{name}'");
                return ExitCode::from(1);
            }
        }
    };
    let mut failed = false;
    for rec in &records {
        match restore_one(rec, &cfg.agents) {
            Ok(o) => println!("{}: {}", rec.name, o.label()),
            Err(e) => {
                failed = true;
                println!("{}: error: {e}", rec.name);
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Set on the detached child that restarts the client's own session (see [`cmd_restart`]).
const RESTART_DETACHED_ENV: &str = "CDAMUX_RESTART_DETACHED";

fn cmd_restart(name: Option<String>, all: bool, force: bool) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let live = list_panes_all();
    let records = if all {
        session_registry::load_all()
    } else {
        let name = name.unwrap_or_default();
        match session_registry::load(&name) {
            Some(r) if live.contains_key(&name) => vec![r],
            Some(_) => {
                println!("{name}: skipped (not live; cdamux restore {name})");
                return ExitCode::SUCCESS;
            }
            None => {
                eprintln!("restart: no record for session '{name}'");
                return ExitCode::from(1);
            }
        }
    };
    // The detached child restarts the session it runs in: no second hand-off.
    let current = if std::env::var_os(RESTART_DETACHED_ENV).is_some() {
        None
    } else {
        current_tmux_session()
    };
    let mut failed = false;
    for (rec, step) in restart_plan(records, &live, current.as_deref(), force) {
        let line = match step {
            Err(reason) => format!("skipped ({reason})"),
            // Killing our own session hangs up this process: a detached copy finishes the job.
            Ok(()) if current.as_deref() == Some(rec.name.as_str()) => {
                println!("{}: restarting in the background (current session)", rec.name);
                match restart_detached(&rec.name) {
                    Ok(()) => continue,
                    Err(e) => format!("error: {e}"),
                }
            }
            Ok(()) => match kill_tmux_session(&rec.name) {
                Err(e) => format!("error: kill: {e}"),
                Ok(()) => match restore_one(&rec, &cfg.agents) {
                    Ok(o) => format!("restarted, {}", o.label()),
                    Err(e) => format!("error: killed, restore failed: {e}"),
                },
            },
        };
        failed |= line.starts_with("error");
        println!("{}: {line}", rec.name);
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `cdamux restart <name> --force` in its own process group under `nohup`, so the
/// tmux hang-up of `name`'s panes cannot stop it; output goes to `restart.log` next to
/// the registry (`--force`: the caller already checked busy).
fn restart_detached(name: &str) -> io::Result<()> {
    let log = session_registry::registry_dir()
        .and_then(|d| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(d.with_file_name("restart.log"))
                .ok()
        })
        .map_or_else(Stdio::null, Stdio::from);
    Command::new("nohup")
        .arg(std::env::current_exe()?)
        .args(["restart", name, "--force"])
        .env(RESTART_DETACHED_ENV, "1")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

fn cmd_spawn(agent: Option<String>, path: Option<String>, interactive: bool) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let (path_buf, agent_name) = if interactive {
        match interactive_spawn(&cfg) {
            Ok(v) => v,
            Err(()) => return ExitCode::from(1),
        }
    } else if path.is_none() && agent.is_none() {
        eprintln!("spawn: provide --path and --agent, or use -i / --interactive");
        return ExitCode::from(1);
    } else {
        let agent_name = agent.unwrap_or_else(|| default_agent_name(&cfg.agents));
        let path_buf = match path {
            Some(p) => PathBuf::from(p),
            None => match interactive_pick_path(&cfg) {
                Ok(p) => p,
                Err(()) => return ExitCode::from(1),
            },
        };
        (path_buf, agent_name)
    };

    spawn_with_path_and_agent(&cfg, path_buf, agent_name)
}

fn spawn_with_path_and_agent(
    cfg: &cdamux_core::Config,
    path_buf: PathBuf,
    agent_name: String,
) -> ExitCode {
    let command = match resolve_agent_command(&cfg.agents, &agent_name) {
        Ok(c) => c,
        Err(SpawnError::UnknownAgent(a)) => {
            eprintln!(
                "spawn: unknown agent '{a}' — add [agents.{a}] in config (cursor, opencode, claude, pi)"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("spawn: {e}");
            return ExitCode::from(1);
        }
    };

    let session_name = unique_session_name(&session_name_for_path(&path_buf));
    match spawn_tmux_session_with_layout(
        &session_name,
        &path_buf,
        &command,
        PaneLayout::Single,
        true,
        Some(agent_name.as_str()),
        None,
    ) {
        Ok(o) => {
            println!("spawned session '{}' in {}{}", session_name, path_buf.display(), o.note());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("spawn: {e}");
            ExitCode::from(1)
        }
    }
}

fn interactive_spawn(cfg: &cdamux_core::Config) -> Result<(PathBuf, String), ()> {
    let path_buf = interactive_pick_path(cfg)?;
    let agent = interactive_pick_agent(cfg)?;
    Ok((path_buf, agent))
}

fn interactive_pick_path(cfg: &cdamux_core::Config) -> Result<PathBuf, ()> {
    let projects = discover_projects(&cfg.projects);
    let mut lines: Vec<String> = projects
        .iter()
        .map(|p| format!("{}\t{}\t{}", p.root_label, p.name, p.path.display()))
        .collect();
    lines.push(CUSTOM_PATH_ITEM.to_string());

    let path = skim_pick("project> ", &lines.join("\n"))?;
    if path == CUSTOM_PATH_ITEM {
        eprint!("path> ");
        io::stderr().flush().ok();
        let mut stdin = io::stdin().lock();
        let mut buf = String::new();
        if stdin.read_line(&mut buf).is_err() {
            return Err(());
        }
        Ok(PathBuf::from(buf.trim()))
    } else {
        let parts: Vec<&str> = path.split('\t').collect();
        let p = parts.last().map(|s| *s).unwrap_or(path.as_str());
        Ok(PathBuf::from(p))
    }
}

fn interactive_pick_agent(cfg: &cdamux_core::Config) -> Result<String, ()> {
    let agents = configured_agent_ids(&cfg.agents);
    let agent_line = agents.join("\n");
    skim_pick("agent> ", &agent_line)
}

fn cmd_notes_list() -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let projects = discover_projects(&cfg.projects);
    let paths = projects.iter().map(|p| p.path.clone()).collect::<Vec<_>>();
    let mut paths = paths;
    if let Ok(sessions) = list_tmux_sessions() {
        for s in sessions {
            if !s.path.is_empty() {
                let p = PathBuf::from(&s.path);
                if !paths.iter().any(|x| x == &p) {
                    paths.push(p);
                }
            }
        }
    }
    let refs = note_refs_for_projects(&cfg.notes, &paths);
    for r in refs {
        let exists = if r.note_path.exists() { "yes" } else { "no" };
        println!(
            "{}\t{}\texists={}",
            r.project_path.display(),
            r.note_path.display(),
            exists
        );
    }
    ExitCode::SUCCESS
}

fn resolve_notes_project(
    path: Option<String>,
    session: Option<String>,
) -> Result<PathBuf, ExitCode> {
    if let Some(p) = path {
        return Ok(PathBuf::from(p));
    }
    if let Some(name) = session {
        return session_cwd(&name).map(PathBuf::from).ok_or_else(|| {
            eprintln!("notes: unknown session '{name}'");
            ExitCode::from(1)
        });
    }
    if let Ok(sessions) = list_tmux_sessions() {
        if sessions.len() == 1 {
            return Ok(PathBuf::from(&sessions[0].path));
        }
        if let Ok(current) = current_tmux_session_name() {
            if let Some(s) = sessions.iter().find(|x| x.name == current) {
                return Ok(PathBuf::from(&s.path));
            }
        }
    }
    eprintln!("notes: pass --path or --session (or run with a single tmux session)");
    Err(ExitCode::from(1))
}

fn session_cwd(session: &str) -> Option<String> {
    list_tmux_sessions()
        .ok()?
        .into_iter()
        .find(|s| s.name == session)
        .map(|s| s.path)
}

fn current_tmux_session_name() -> Result<String, ()> {
    let out = Command::new("tmux")
        .args(["display-message", "-p", "#{session_name}"])
        .output()
        .map_err(|_| ())?;
    if !out.status.success() {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn cmd_notes_show(path: Option<String>, session: Option<String>, render: bool) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let project = match resolve_notes_project(path, session) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let note_path = resolve_note_path(&cfg.notes, &project);
    if !note_path.exists() {
        eprintln!("notes: no file at {}", note_path.display());
        return ExitCode::from(1);
    }
    let body = match std::fs::read_to_string(&note_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("notes: read {}: {e}", note_path.display());
            return ExitCode::from(1);
        }
    };
    if render {
        if let Err(e) = render_markdown_stdout(&body) {
            eprintln!("notes: render: {e}");
            return ExitCode::from(1);
        }
    } else if let Err(e) = writeln!(io::stdout(), "{body}") {
        eprintln!("notes: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn cmd_notes_edit(path: Option<String>, session: Option<String>) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let project = match resolve_notes_project(path, session) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let note_path = resolve_note_path(&cfg.notes, &project);
    if let Err(e) = ensure_note_file(&note_path) {
        eprintln!("notes: create {}: {e}", note_path.display());
        return ExitCode::from(1);
    }
    let editor = notes_editor(&cfg.notes);
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "exec {} {}",
            editor,
            shell_escape(note_path.to_string_lossy().as_ref())
        ))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("notes: editor: {e}");
            ExitCode::from(1)
        }
    }
}

fn render_markdown_stdout(md: &str) -> io::Result<()> {
    let rendered = cdamux_core::render_markdown_terminal(md);
    write!(io::stdout(), "{rendered}")?;
    if !rendered.ends_with('\n') {
        writeln!(io::stdout())?;
    }
    Ok(())
}

fn notes_editor(notes: &cdamux_core::NotesConfig) -> String {
    if !notes.editor.is_empty() {
        return notes.editor.clone();
    }
    std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string())
}

fn skim_pick(prompt: &str, input: &str) -> Result<String, ()> {
    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input.to_string()));
    let options = SkimOptionsBuilder::default()
        .prompt(prompt.into())
        .height("100%".into())
        .build()
        .unwrap();
    let selected = match Skim::run_with(&options, Some(items)) {
        Some(out) => out.selected_items,
        None => return Err(()),
    };
    if selected.is_empty() {
        return Err(());
    }
    Ok(selected[0].output().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_send(args: &[&str]) -> (String, Vec<String>, bool) {
        let cli = Cli::try_parse_from(args).expect("parse");
        match cli.command {
            Some(Commands::Send { session, message, enter }) => (session, message, enter),
            _ => panic!("expected send"),
        }
    }

    #[test]
    fn restore_takes_a_name_or_all() {
        for args in [&["cdamux", "restore", "c-a"][..], &["cdamux", "restore", "--all"]] {
            assert!(matches!(Cli::try_parse_from(args).unwrap().command, Some(Commands::Restore { .. })));
        }
        assert!(Cli::try_parse_from(["cdamux", "restore"]).is_err());
        assert!(Cli::try_parse_from(["cdamux", "restore", "c-a", "--all"]).is_err());
    }

    #[test]
    fn restart_takes_a_name_or_all_and_force() {
        let cli = Cli::try_parse_from(["cdamux", "restart", "--all", "--force"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Restart { all: true, force: true, .. })));
        assert!(Cli::try_parse_from(["cdamux", "restart", "c-a"]).is_ok());
        assert!(Cli::try_parse_from(["cdamux", "restart"]).is_err());
        assert!(Cli::try_parse_from(["cdamux", "restart", "c-a", "--all"]).is_err());
    }

    #[test]
    fn send_enter_works_before_and_after_text() {
        let after = parse_send(&["cdamux", "send", "t1", "echo hi", "--enter"]);
        assert_eq!(after, ("t1".into(), vec!["echo hi".to_string()], true));
        let before = parse_send(&["cdamux", "send", "--enter", "t1", "echo", "hi"]);
        assert_eq!(before, ("t1".into(), vec!["echo".to_string(), "hi".to_string()], true));
        let after_words = parse_send(&["cdamux", "send", "t1", "echo", "hi", "--enter"]);
        assert_eq!(after_words.1, ["echo", "hi"]);
        assert!(after_words.2);
    }
}
