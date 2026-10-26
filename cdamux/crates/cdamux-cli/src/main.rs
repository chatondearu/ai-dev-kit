//! cdamux — CLI for agent session orchestration on tmux.

mod tui;

use cdamux_core::{
    config_path, configured_agent_ids, default_agent_name, discover_projects, grouped_for_list,
    list_tmux_sessions, load_config, resolve_agent_command, session_name_for_path,
    spawn_tmux_session, AttachMode, GroupBy, SpawnError, SpawnOutcome,
};
use clap::{Parser, Subcommand};
use skim::prelude::*;
use std::io::{self, BufRead, Cursor, Write};
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
    /// List tmux sessions (daemon enrichment when cdamuxd runs).
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

    match config_path() {
        Some(p) => println!("config: {}", p.display()),
        None => eprintln!("config: cannot resolve XDG path"),
    }

    let mut doctor_ok = true;
    if let Ok(cfg) = load_config() {
        println!("roots: {}", cfg.projects.roots.len());
        println!("group_by: {:?}", cfg.sessions.group_by);
        let agents = configured_agent_ids(&cfg.agents);
        println!("agents (configured): {}", agents.join(", "));
        for id in ["cursor", "opencode", "claude", "pi"] {
            let configured = agent_configured(&cfg.agents, id);
            let exe = agent_executable_name(&cfg.agents, id);
            let on_path = command_on_path(&exe);
            let tag = if configured { "configured" } else { "optional" };
            if configured && !on_path {
                doctor_ok = false;
                eprintln!("agent {id}: missing on PATH ({exe}) [{tag}]");
            } else if on_path {
                println!("agent {id}: ok ({exe}) [{tag}]");
            } else {
                println!("agent {id}: not on PATH ({exe}) [{tag}]");
            }
        }
    }

    match Command::new("tailscale").arg("version").output() {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout);
            println!("tailscale: {}", ver.lines().next().unwrap_or("ok"));
        }
        _ => println!("tailscale: not installed (optional)"),
    }

    if let Some(sock) = cdamux_core::socket_path() {
        let status = if sock.exists() {
            "running (socket present)"
        } else {
            "not running"
        };
        println!("cdamuxd: {} ({})", status, sock.display());
    }

    if !doctor_ok {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
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

fn command_on_path(name: &str) -> bool {
    let bin = name.split_whitespace().next().unwrap_or(name);
    Command::new("sh")
        .args(["-c", &format!("command -v -- {}", shell_escape(bin))])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/') {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn cmd_list(json: bool, group_by: GroupBy) -> ExitCode {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let grouped = grouped_for_list(
        group_by,
        &cfg.projects,
        cfg.daemon.idle_seconds_for_needs_input,
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
            println!("{}\t{}\t{}", s.name, s.path, format_state(s.state));
        }
    }
    ExitCode::SUCCESS
}

fn format_state(state: cdamux_core::SessionState) -> &'static str {
    use cdamux_core::SessionState;
    match state {
        SessionState::Idle => "idle",
        SessionState::Active => "active",
        SessionState::NeedsInput => "needs_input",
    }
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

fn cmd_attach(session: &str) -> ExitCode {
    tmux_attach_session(session)
}

pub(crate) fn tmux_attach_session(session: &str) -> ExitCode {
    let attach_mode = load_config()
        .map(|c| c.sessions.attach_mode)
        .unwrap_or(AttachMode::Mirror);
    let nested = std::env::var_os("TMUX").is_some();
    let status = if nested {
        Command::new("tmux")
            .args(["switch-client", "-t", session])
            .status()
    } else {
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

pub(crate) fn cmd_spawn_interactive(cfg: &cdamux_core::Config) -> ExitCode {
    let (path_buf, agent_name) = match interactive_spawn(cfg) {
        Ok(v) => v,
        Err(()) => return ExitCode::from(1),
    };
    spawn_with_path_and_agent(cfg, path_buf, agent_name)
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

    let session_name = session_name_for_path(&path_buf);
    match spawn_tmux_session(
        &session_name,
        &path_buf,
        &command,
        cfg.sessions.attach_mode,
    ) {
        Ok(SpawnOutcome::ReusedExisting) => {
            println!("reusing existing session '{}'", session_name);
            ExitCode::SUCCESS
        }
        Ok(SpawnOutcome::Created) => {
            println!("spawned session '{}' in {}", session_name, path_buf.display());
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
