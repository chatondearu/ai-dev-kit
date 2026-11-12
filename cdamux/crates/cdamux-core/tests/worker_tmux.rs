//! Worker spawn|ls|peek + msg targeting on a throwaway tmux server.

use cdamux_core::{
    env_lock, list_workers, msg_send, peek_worker, spawn_worker, state_get, state_set, AgentsConfig,
    AgentEntry, StateKind, WorkerSpawnRequest, DEFAULT_NUDGE_QUIET_SECS,
};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-worker-test";

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn wait_for_pane(session: &str, needle: &str) -> String {
    let start = Instant::now();
    loop {
        let text = tmux(&[
            "capture-pane",
            "-p",
            "-J",
            "-t",
            &format!("={session}:"),
        ]);
        if text.contains(needle) || start.elapsed() > Duration::from_secs(5) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn spawn_ls_peek_and_msg_by_name() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let Some(git) = which("git") else {
        eprintln!("git not on PATH: skipped");
        return;
    };
    let _ = git;

    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(
        &wrapper,
        format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n"),
    )
    .unwrap();
    // Stub agent CLI: keep the pane alive so peek / msg paste work.
    let stub = bin.join("claude");
    std::fs::write(&stub, "#!/bin/sh\nexec sleep 30\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    assert!(Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "init", "-b", "main"])
        .status()
        .unwrap()
        .success());
    std::fs::write(repo.join("README"), "x").unwrap();
    assert!(Command::new("git")
        .args(["-C", repo.to_str().unwrap(), "add", "README"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args([
            "-C",
            repo.to_str().unwrap(),
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "init",
        ])
        .status()
        .unwrap()
        .success());

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");

    assert!(Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            "c-orch",
            "-c",
            repo.to_str().unwrap(),
            "sh",
            "-c",
            "sleep 60",
        ])
        .status()
        .unwrap()
        .success());

    state_set(
        StateKind::Agents,
        "a-orch1",
        &serde_json::json!({
            "id": "a-orch1",
            "client": "claude",
            "host": "c-orch",
            "role": "orchestrator",
            "parent": null,
            "run": null,
            "status": "idle"
        }),
    )
    .unwrap();

    let agents = AgentsConfig {
        default: "claude".into(),
        claude: Some(AgentEntry {
            command: "claude".into(),
            resume: None,
        }),
        cursor: None,
        opencode: None,
        pi: None,
    };
    let prompt = tmp.path().join("prompt.txt");
    std::fs::write(&prompt, "hello-worker-spawn").unwrap();

    let wt_root = tmp.path().join("wts");
    let out = spawn_worker(WorkerSpawnRequest {
        name: "api",
        caller_id: "a-orch1",
        agent: None,
        prompt_file: Some(&prompt),
        here: false,
        worktree_root: Some(&wt_root),
        agents: &agents,
        cwd: &repo,
    })
    .expect("spawn worker");
    assert!(!out.agent_id.is_empty(), "missing agent id");
    assert_eq!(out.name, "api");
    assert!(out.path.starts_with(&wt_root));
    assert!(out.run.starts_with("r-"));

    let orch = state_get(StateKind::Agents, "a-orch1").unwrap().unwrap();
    assert_eq!(orch["run"], out.run);

    let worker = state_get(StateKind::Agents, &out.agent_id).unwrap().unwrap();
    assert_eq!(worker["role"], "worker");
    assert_eq!(worker["parent"], "a-orch1");
    assert_eq!(worker["name"], "api");
    assert_eq!(worker["run"], out.run);
    assert_eq!(worker["host"], out.host);

    let rows = list_workers("a-orch1").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "api");

    let _peek = peek_worker("a-orch1", "api", 20).unwrap();

    let pasted = msg_send(
        "a-orch1",
        "api",
        "relay",
        "hello-by-name",
        None,
        DEFAULT_NUDGE_QUIET_SECS,
        None,
    )
    .unwrap();
    assert!(pasted.pasted);
    let pane = wait_for_pane(&out.host, "hello-by-name");
    assert!(
        pane.contains("hello-by-name"),
        "worker pane missing paste: {pane}"
    );

    let sock = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}
