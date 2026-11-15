//! `cdamux orch new` on a throwaway tmux server + temp CDAMUX_HOME.

use cdamux_core::{
    env_lock, orch_boot_prompt, orch_boot_prompt_global, spawn_orchestrator, state_get,
    AgentsConfig, AgentEntry, OrchNewRequest, StateKind, ORCH_SCOPE_GLOBAL,
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

fn wait_for_file(path: &std::path::Path) -> String {
    let start = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() || start.elapsed() > Duration::from_secs(5) {
                return text;
            }
        }
        if start.elapsed() > Duration::from_secs(5) {
            return String::new();
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
    let stub_out = tmp.path().join("claude.out");
    let stub = bin.join("claude");
    // Record argv (prompt file expanded by the shell) and CDA_ROLE from the pane env prefix.
    std::fs::write(
        &stub,
        format!(
            "#!/bin/sh\nprintf 'ARGV:%s\\nCDA_ROLE=%s\\n' \"$*\" \"${{CDA_ROLE-}}\" > {}\nexec sleep 30\n",
            stub_out.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let cwd = tmp.path().join("proj");
    std::fs::create_dir(&cwd).unwrap();

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    let home = tmp.path().join("cdamux");
    std::env::set_var("CDAMUX_HOME", &home);
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
        global: false,
    })
    .expect("orch new");

    assert!(!out.agent_id.is_empty(), "missing agent id");
    assert!(!out.reused);
    assert!(out.run.starts_with("r-"), "run={}", out.run);
    assert_eq!(out.client, "claude");

    let agent = state_get(StateKind::Agents, &out.agent_id).unwrap().unwrap();
    assert_eq!(agent["role"], "orchestrator");
    assert_eq!(agent["run"], out.run);
    assert_eq!(agent["host"], out.host);
    assert!(agent.get("scope").is_none() || agent["scope"].is_null());

    let run = state_get(StateKind::Runs, &out.run).unwrap().unwrap();
    assert_eq!(run["parent"], out.agent_id);

    let prompt_path = home.join("prompts").join(format!("{}.md", out.host));
    assert!(
        prompt_path.is_file(),
        "missing prompt file {}",
        prompt_path.display()
    );
    let prompt_body = std::fs::read_to_string(&prompt_path).unwrap();
    assert_eq!(prompt_body.trim(), orch_boot_prompt());

    let stub_log = wait_for_file(&stub_out);
    assert!(
        stub_log.contains("CDA_ROLE=orchestrator"),
        "orchestrator pane missing CDA_ROLE; stub_log={stub_log}"
    );
    assert!(
        stub_log.contains("cda-orchestrator"),
        "boot prompt not on CLI argv; stub_log={stub_log}; file={}",
        prompt_path.display()
    );

    let sock = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}

#[test]
fn orch_new_global_sets_scope_and_reuses_live() {
    const SOCK: &str = "cdamux-orch-global-test";
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
        format!("#!/bin/sh\nexec {real_tmux} -L {SOCK} -f /dev/null \"$@\"\n"),
    )
    .unwrap();
    let stub = bin.join("claude");
    std::fs::write(&stub, "#!/bin/sh\nexec sleep 30\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    let home = tmp.path().join("cdamux");
    std::env::set_var("CDAMUX_HOME", &home);
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

    let dummy = tmp.path().join("ignored-cwd");
    std::fs::create_dir(&dummy).unwrap();

    let first = spawn_orchestrator(OrchNewRequest {
        path: &dummy,
        agent: Some("claude"),
        agents: &agents,
        switch_to_session: false,
        global: true,
    })
    .expect("global orch new");

    assert!(!first.reused);
    assert!(first.host.starts_with("c-global"), "host={}", first.host);
    assert_eq!(first.path, home.join("global"));
    let agent = state_get(StateKind::Agents, &first.agent_id).unwrap().unwrap();
    assert_eq!(agent["role"], "orchestrator");
    assert_eq!(agent["scope"], ORCH_SCOPE_GLOBAL);

    let prompt_path = home.join("prompts").join(format!("{}.md", first.host));
    let prompt_body = std::fs::read_to_string(&prompt_path).unwrap();
    assert_eq!(prompt_body.trim(), orch_boot_prompt_global());

    let second = spawn_orchestrator(OrchNewRequest {
        path: &dummy,
        agent: Some("claude"),
        agents: &agents,
        switch_to_session: false,
        global: true,
    })
    .expect("second global orch new");

    assert!(second.reused);
    assert_eq!(second.host, first.host);
    assert_eq!(second.agent_id, first.agent_id);

    let sock = Command::new("tmux")
        .args(["display-message", "-p", "#{socket_path}"])
        .output()
        .expect("tmux");
    let sock = String::from_utf8_lossy(&sock.stdout);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}
