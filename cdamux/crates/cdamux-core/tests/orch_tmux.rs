//! `cdamux orch new` on a throwaway tmux server + temp CDAMUX_HOME.

use cdamux_core::{
    env_lock, orch_boot_prompt, spawn_orchestrator, state_get, AgentsConfig, AgentEntry,
    OrchNewRequest, StateKind, ORCHESTRATOR_SKILL,
};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-orch-test";

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
fn orch_new_sets_role_run_and_boot_prompt() {
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
        format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n"),
    )
    .unwrap();
    let stub = bin.join("claude");
    std::fs::write(&stub, "#!/bin/sh\nexec sleep 30\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let cwd = tmp.path().join("proj");
    std::fs::create_dir(&cwd).unwrap();

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");

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

    let out = spawn_orchestrator(OrchNewRequest {
        path: &cwd,
        agent: Some("claude"),
        agents: &agents,
        switch_to_session: false,
    })
    .expect("orch new");

    assert!(!out.agent_id.is_empty(), "missing agent id");
    assert!(out.run.starts_with("r-"), "run={}", out.run);
    assert_eq!(out.client, "claude");

    let agent = state_get(StateKind::Agents, &out.agent_id).unwrap().unwrap();
    assert_eq!(agent["role"], "orchestrator");
    assert_eq!(agent["run"], out.run);
    assert_eq!(agent["host"], out.host);

    let run = state_get(StateKind::Runs, &out.run).unwrap().unwrap();
    assert_eq!(run["parent"], out.agent_id);

    let needle = ORCHESTRATOR_SKILL;
    let pane = wait_for_pane(&out.host, needle);
    assert!(
        pane.contains(needle),
        "boot prompt missing skill name; pane={pane}; boot={}",
        orch_boot_prompt()
    );

    let sock = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}
