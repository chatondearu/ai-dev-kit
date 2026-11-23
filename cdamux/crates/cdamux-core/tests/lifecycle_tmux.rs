//! W4 lifecycle on a throwaway tmux server: CDAMUX_HOME reaches every spawned pane,
//! workers of a dead orchestrator free the cap, `worker kill [--subtree]` is permanent.

use cdamux_core::{
    env_lock, kill_worker, list_orchestrators, read_events, restore_targets, session_registry,
    spawn_orchestrator, spawn_worker, state_get, state_set, AgentEntry, AgentsConfig, OrchError,
    OrchNewRequest, StateKind, WorkerError, WorkerSpawnRequest,
};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-lifecycle-test";

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn tmux(args: &[&str]) -> (bool, String) {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn has_session(name: &str) -> bool {
    tmux(&["has-session", "-t", &format!("={name}")]).0
}

fn wait_for_pane(session: &str, needle: &str) -> String {
    let start = Instant::now();
    loop {
        let (_, text) = tmux(&["capture-pane", "-p", "-J", "-t", &format!("={session}:")]);
        if text.contains(needle) || start.elapsed() > Duration::from_secs(5) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn git(repo: &Path, args: &[&str]) {
    assert!(Command::new("git").arg("-C").arg(repo).args(args).status().unwrap().success());
}

fn orch(id: &str, host: &str, cwd: &Path) {
    assert!(tmux(&["new-session", "-d", "-s", host, "-c", cwd.to_str().unwrap(), "sleep", "120"]).0);
    state_set(
        StateKind::Agents,
        id,
        &serde_json::json!({"id": id, "client": "claude", "host": host, "role": "orchestrator", "status": "idle"}),
    )
    .unwrap();
}

#[test]
fn home_propagation_cap_and_permanent_kill() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    if which("git").is_none() {
        eprintln!("git not on PATH: skipped");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n")).unwrap();
    let stub = bin.join("claude");
    std::fs::write(&stub, "#!/bin/sh\nprintf 'home=%s\\n' \"$CDAMUX_HOME\"\nexec sleep 60\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    for f in [&wrapper, &stub] {
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    let home = tmp.path().join("state home");
    std::env::set_var("CDAMUX_HOME", &home);
    std::env::remove_var("TMUX");

    let agents = AgentsConfig {
        default: "claude".into(),
        claude: Some(AgentEntry { command: "claude".into(), resume: None }),
        cursor: None,
        opencode: None,
        pi: None,
    };
    let wt_root = tmp.path().join("wts");
    let spawn = |caller: &str, name: &str, cap: u32, agent: Option<&str>| {
        spawn_worker(WorkerSpawnRequest {
            name,
            caller_id: caller,
            agent,
            prompt_file: None,
            here: false,
            repo: None,
            worktree_root: Some(&wt_root),
            agents: &agents,
            cwd: &repo,
            max_workers: cap,
            min_free_gib: 0,
        })
    };

    orch("a-orch1", "c-orch1", &repo);
    let w1 = spawn("a-orch1", "w1", 2, None).expect("w1");

    // H5 / #391: the pane env and the agent process both see the spawner's home.
    let (_, env) = tmux(&["show-environment", "-t", &w1.host, "CDAMUX_HOME"]);
    assert_eq!(env.trim(), format!("CDAMUX_HOME={}", home.display()));
    let pane = wait_for_pane(&w1.host, "home=/");
    assert!(pane.contains(&format!("home={}", home.display())), "agent env: {pane}");
    let spawned = read_events(0)
        .unwrap()
        .into_iter()
        .any(|v| v["type"] == "lifecycle" && v["event"] == "spawn" && v["agent"] == w1.agent_id.as_str());
    assert!(spawned, "missing spawn lifecycle item");

    // M10: an unknown agent fails before any worktree / branch exists.
    let bad = spawn("a-orch1", "bad", 0, Some("cursor"));
    assert!(bad.is_err());
    assert!(!wt_root.join(&w1.run).join("bad").exists());

    let w2 = spawn("a-orch1", "w2", 2, None).expect("w2");
    assert!(matches!(spawn("a-orch1", "w3", 2, None), Err(WorkerError::Cap { live: 2, cap: 2 })));

    // H2: the orchestrator dies; a new one gets the slots its workers held.
    assert!(tmux(&["kill-session", "-t", "=c-orch1"]).0);
    orch("a-orch2", "c-orch2", &repo);
    let w3 = spawn("a-orch2", "w3", 2, None).expect("dead orchestrator's workers must not hold the cap");

    // Permanent kill of an own worker: session gone, finalized, never restored.
    assert!(matches!(kill_worker(Some("a-orch2"), "w1", false), Err(WorkerError::NotFound(_))), "not its worker");
    // Q23: an orchestrator only kills a finished worker (done / idle / error).
    assert!(kill_worker(Some("a-orch2"), "w3", false).is_err(), "no status yet: refused");
    assert!(has_session(&w3.host));
    cdamux_core::state_append(StateKind::Agents, &w3.agent_id, &serde_json::json!({"status": "idle"})).unwrap();
    let killed = kill_worker(Some("a-orch2"), "w3", false).unwrap();
    assert_eq!(killed.len(), 1);
    assert!(!has_session(&w3.host));
    let rec = state_get(StateKind::Agents, &w3.agent_id).unwrap().unwrap();
    assert_eq!((rec["status"].as_str(), rec["host"].as_str()), (Some("error"), Some("")));
    assert!(session_registry::load(&w3.host).is_none());
    assert!(restore_targets(&w3.host).is_none(), "restore must not respawn a killed worker");
    let count = |event: &str, agent: &str| {
        read_events(0)
            .unwrap()
            .into_iter()
            .filter(|v| v["type"] == "lifecycle" && v["event"] == event && v["agent"] == agent)
            .count()
    };
    assert_eq!((count("finalize", &w3.agent_id), count("kill", &w3.agent_id)), (1, 1));

    // A ghost record whose host is now another agent's live session: finalized only,
    // the live session survives (a-yi5om on c-global).
    state_set(
        StateKind::Agents,
        "a-ghost",
        &serde_json::json!({"id": "a-ghost", "role": "orchestrator", "host": "c-orch2", "status": "needs_input"}),
    )
    .unwrap();
    assert!(tmux(&["set-option", "-t", "c-orch2", "@cdamux_id", "a-orch2"]).0);
    kill_worker(None, "a-ghost", false).unwrap();
    assert!(has_session("c-orch2"), "the live orchestrator's session must survive");
    let ghost = state_get(StateKind::Agents, "a-ghost").unwrap().unwrap();
    assert_eq!((ghost["status"].as_str(), ghost["host"].as_str()), (Some("error"), Some("")));
    assert_eq!(state_get(StateKind::Agents, "a-orch2").unwrap().unwrap()["host"], "c-orch2");
    // Two records on one host: a human kill by host name is refused.
    state_set(StateKind::Agents, "a-dup", &serde_json::json!({"id": "a-dup", "host": "c-orch2"})).unwrap();
    assert!(kill_worker(None, "c-orch2", false).is_err());

    // A human kills the dead orchestrator's whole subtree.
    let killed = kill_worker(None, "a-orch1", true).unwrap();
    let ids: Vec<&str> = killed.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert_eq!(ids.last(), Some(&"a-orch1"));
    assert!(!has_session(&w1.host) && !has_session(&w2.host));

    let (_, sock) = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
    std::env::remove_var("CDAMUX_HOME");
    std::env::remove_var("TMUX_TMPDIR");
}

#[test]
fn orch_new_refuses_a_second_live_orchestrator_and_clears_stale_host_claims() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET}-orch -f /dev/null \"$@\"\n")).unwrap();
    let stub = bin.join("claude");
    std::fs::write(&stub, "#!/bin/sh\nexec sleep 60\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    for f in [&wrapper, &stub] {
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let proj = tmp.path().join("proj");
    std::fs::create_dir(&proj).unwrap();

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");
    // Keep a server up so has-session answers.
    assert!(tmux(&["new-session", "-d", "-s", "keep", "sleep", "60"]).0);

    // A stale record still naming the session the new orchestrator will get.
    state_set(
        StateKind::Agents,
        "a-ghost",
        &serde_json::json!({"id": "a-ghost", "role": "orchestrator", "host": "c-proj", "status": "needs_input"}),
    )
    .unwrap();

    let agents = AgentsConfig {
        default: "claude".into(),
        claude: Some(AgentEntry { command: "claude".into(), resume: None }),
        cursor: None,
        opencode: None,
        pi: None,
    };
    let req = OrchNewRequest {
        path: &proj,
        agent: None,
        agents: &agents,
        switch_to_session: false,
        global: false,
        parent_agent_id: None,
    };
    let first = spawn_orchestrator(req.clone()).expect("first orchestrator");
    assert_eq!(first.host, "c-proj");
    let ghost = state_get(StateKind::Agents, "a-ghost").unwrap().unwrap();
    assert_eq!((ghost["status"].as_str(), ghost["host"].as_str()), (Some("error"), Some("")));
    let live: Vec<String> = list_orchestrators().into_iter().filter(|e| e.live).map(|e| e.id).collect();
    assert_eq!(live, [first.agent_id.clone()], "only the new orchestrator is live on c-proj");

    match spawn_orchestrator(req) {
        Err(OrchError::Exists { id, host, .. }) => assert_eq!((id, host), (first.agent_id, first.host)),
        other => panic!("second orchestrator for the same project must be refused: {other:?}"),
    }

    let (_, sock) = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
    std::env::remove_var("CDAMUX_HOME");
    std::env::remove_var("TMUX_TMPDIR");
}
