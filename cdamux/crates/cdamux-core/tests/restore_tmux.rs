//! Spawn → kill-session → restore on a throwaway tmux server (cdamux/AGENTS.md
//! § Testing against tmux). Tests in this binary set process env vars.

use cdamux_core::{
    env_lock, restore_one, restore_targets, session_registry, spawn_tmux_session_ex,
    spawn_tmux_session_with_layout, state_append, state_get, AgentEntry, AgentsConfig, PaneLayout,
    RestoreOutcome, SpawnExtra, StateKind,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-restore-test";

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn wait_for_pane(session: &str, needle: &str) -> String {
    let start = Instant::now();
    loop {
        // All panes: after a split the active pane is the shell, not the agent.
        let text: String = tmux(&["list-panes", "-t", &format!("={session}:"), "-F", "#{pane_id}"])
            .lines()
            .map(|pane| tmux(&["capture-pane", "-p", "-J", "-t", pane]))
            .collect();
        if text.contains(needle) || start.elapsed() > Duration::from_secs(5) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn spawn_kill_restore_resumes_claude_conversation() {
    let _env = env_lock();
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    // Wrapper so every tmux call (ours and cdamux's) hits a private `-L` server under tmp.
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {} -L {SOCKET} -f /dev/null \"$@\"\n", real_tmux)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("XDG_STATE_HOME", tmp.path().join("state"));
    std::env::remove_var("CDAMUX_HOME");
    std::env::set_var("SHELL", "/bin/sh");
    std::env::remove_var("TMUX");

    let agents = AgentsConfig {
        default: "claude".into(),
        // `echo` stands in for claude: the typed command line shows in the pane.
        claude: Some(AgentEntry {
            command: "echo claude".into(),
            resume: None,
        }),
        ..AgentsConfig::default()
    };
    let cwd = tmp.path();
    let name = "c-restore";

    spawn_tmux_session_with_layout(name, cwd, "echo claude", PaneLayout::Horizontal, false, Some("claude"), Some("c-main"))
        .expect("spawn");
    let before = session_registry::load(name).expect("recorded");
    let id = before.agent_session_id.clone().expect("pre-assigned id");
    let cdamux_id = before.cdamux_id.clone().expect("cdamux_id");
    assert_eq!(
        tmux(&["show-options", "-v", "-t", name, "@cdamux_id"]).trim(),
        cdamux_id.as_str()
    );
    let agent_path = PathBuf::from(std::env::var_os("XDG_STATE_HOME").unwrap())
        .join("cdamux/agents")
        .join(format!("{cdamux_id}.json"));
    let agent_doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&agent_path).expect("agent state file")).unwrap();
    assert_eq!(agent_doc["id"], cdamux_id);
    assert_eq!(agent_doc["host"], name);
    // Parent session has no registry entry → entity parent is null (not the tmux name).
    assert!(agent_doc["parent"].is_null(), "parent={:?}", agent_doc["parent"]);
    state_append(
        StateKind::Agents,
        &cdamux_id,
        &serde_json::json!({"status": "idle", "resume_id": "keep-me"}),
    )
    .expect("append before restore");

    let result = std::panic::catch_unwind(|| {
        assert_eq!(restore_one(&before, &agents).unwrap(), RestoreOutcome::SkippedLive);

        tmux(&["kill-session", "-t", &format!("={name}")]);
        assert!(tmux(&["list-sessions", "-F", "#{session_name}"]).lines().all(|l| l != name));

        let outcome = restore_one(&before, &agents).unwrap();
        assert!(matches!(outcome, RestoreOutcome::Restored(_)), "{outcome:?}");

        let pane = wait_for_pane(name, "--resume");
        assert!(pane.contains(&format!("claude --resume {id}")), "pane: {pane}");
        assert!(!pane.contains("--session-id"), "pane: {pane}");
        assert_eq!(tmux(&["list-panes", "-t", &format!("={name}:"), "-F", "x"]).lines().count(), 2);
        assert_eq!(tmux(&["show-options", "-v", "-t", name, "@cdamux_parent"]).trim(), "c-main");
        assert_eq!(tmux(&["show-options", "-v", "-t", name, "@cdamux_agent"]).trim(), "claude");
        assert_eq!(
            tmux(&["show-options", "-v", "-t", name, "@cdamux_id"]).trim(),
            cdamux_id.as_str()
        );

        let after = session_registry::load(name).unwrap();
        assert_eq!(after.agent_session_id.as_deref(), Some(id.as_str()));
        assert_eq!(after.cdamux_id.as_deref(), Some(cdamux_id.as_str()));
        assert_eq!(after.created_ts, before.created_ts);
        assert!(agent_path.is_file(), "agent file survives restore");
        let after_doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&agent_path).unwrap()).unwrap();
        assert_eq!(after_doc["status"], "idle");
        assert_eq!(after_doc["resume_id"], "keep-me");
        assert_eq!(after_doc["host"], name);
    });

    // Kill only our private server, after checking its socket lives under tmp
    // (no session left → the server already exited, nothing to kill).
    let socket = tmux(&["display", "-p", "-t", &format!("={name}"), "#{socket_path}"]);
    let tmp_real = tmp.path().canonicalize().unwrap();
    let socket = Path::new(socket.trim()).canonicalize().unwrap_or_default();
    if socket.starts_with(&tmp_real) && !socket.to_string_lossy().contains("/default") {
        tmux(&["kill-server"]);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

fn which(cmd: &str) -> Option<String> {
    std::env::var("PATH").ok()?.split(':').map(|d| Path::new(d).join(cmd)).find(|p| p.is_file()).map(|p| p.display().to_string())
}

#[test]
fn restore_orch_restores_workers_with_resume_and_parent() {
    let _env = env_lock();
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(
        &wrapper,
        format!("#!/bin/sh\nexec {} -L {SOCKET}-orch -f /dev/null \"$@\"\n", real_tmux),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("XDG_STATE_HOME", tmp.path().join("state"));
    std::env::remove_var("CDAMUX_HOME");
    std::env::set_var("SHELL", "/bin/sh");
    std::env::remove_var("TMUX");

    let agents = AgentsConfig {
        default: "claude".into(),
        claude: Some(AgentEntry {
            command: "echo claude".into(),
            resume: None,
        }),
        ..AgentsConfig::default()
    };
    let cwd = tmp.path();
    let orch = "c-orch-r";
    let worker = "c-w-r";

    spawn_tmux_session_ex(
        orch,
        cwd,
        "echo claude",
        PaneLayout::Single,
        false,
        Some("claude"),
        None,
        SpawnExtra {
            role: Some("orchestrator"),
            run: Some("r-test1"),
            ..Default::default()
        },
    )
    .expect("spawn orch");
    let orch_rec = session_registry::load(orch).expect("orch recorded");
    let orch_id = orch_rec.cdamux_id.clone().expect("orch cdamux_id");
    let orch_sid = orch_rec.agent_session_id.clone().expect("orch session id");

    spawn_tmux_session_ex(
        worker,
        cwd,
        "echo claude",
        PaneLayout::Single,
        false,
        Some("claude"),
        Some(orch),
        SpawnExtra {
            role: Some("worker"),
            run: Some("r-test1"),
            parent_agent_id: Some(orch_id.as_str()),
            name: Some("w1"),
        },
    )
    .expect("spawn worker");
    let worker_rec = session_registry::load(worker).expect("worker recorded");
    let worker_id = worker_rec.cdamux_id.clone().expect("worker cdamux_id");
    let worker_sid = worker_rec.agent_session_id.clone().expect("worker session id");
    assert_eq!(worker_rec.parent.as_deref(), Some(orch));
    assert_eq!(worker_rec.role, "worker");
    let worker_doc = state_get(StateKind::Agents, &worker_id).unwrap().unwrap();
    assert_eq!(worker_doc["parent"], orch_id);

    let result = std::panic::catch_unwind(|| {
        tmux(&["kill-session", "-t", &format!("={worker}")]);
        tmux(&["kill-session", "-t", &format!("={orch}")]);
        let live = tmux(&["list-sessions", "-F", "#{session_name}"]);
        assert!(!live.lines().any(|l| l == orch || l == worker), "live={live}");

        let targets = restore_targets(orch).expect("orch targets");
        let names: Vec<&str> = targets.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, [orch, worker], "parents first subtree");

        for rec in &targets {
            let outcome = restore_one(rec, &agents).unwrap();
            assert!(matches!(outcome, RestoreOutcome::Restored(_)), "{rec:?} → {outcome:?}");
        }

        let orch_pane = wait_for_pane(orch, "--resume");
        assert!(
            orch_pane.contains(&format!("claude --resume {orch_sid}")),
            "orch pane: {orch_pane}"
        );
        let worker_pane = wait_for_pane(worker, "--resume");
        assert!(
            worker_pane.contains(&format!("claude --resume {worker_sid}")),
            "worker pane: {worker_pane}"
        );
        assert_eq!(tmux(&["show-options", "-v", "-t", worker, "@cdamux_parent"]).trim(), orch);
        assert_eq!(tmux(&["show-options", "-v", "-t", worker, "@cdamux_role"]).trim(), "worker");
        assert_eq!(tmux(&["show-options", "-v", "-t", worker, "@cdamux_run"]).trim(), "r-test1");
        assert_eq!(tmux(&["show-options", "-v", "-t", orch, "@cdamux_run"]).trim(), "r-test1");
        assert_eq!(tmux(&["show-options", "-v", "-t", orch, "@cdamux_role"]).trim(), "orchestrator");
        assert_eq!(
            tmux(&["show-options", "-v", "-t", worker, "@cdamux_id"]).trim(),
            worker_id.as_str()
        );
        let after_worker = state_get(StateKind::Agents, &worker_id).unwrap().unwrap();
        assert_eq!(after_worker["parent"], orch_id);
        assert_eq!(after_worker["host"], worker);
    });

    let socket = tmux(&["display", "-p", "-t", &format!("={orch}"), "#{socket_path}"]);
    let tmp_real = tmp.path().canonicalize().unwrap();
    let socket = Path::new(socket.trim()).canonicalize().unwrap_or_default();
    if socket.starts_with(&tmp_real) && !socket.to_string_lossy().contains("/default") {
        tmux(&["kill-server"]);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
