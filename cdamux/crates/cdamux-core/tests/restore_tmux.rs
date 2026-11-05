//! Spawn → kill-session → restore on a throwaway tmux server (cdamux/AGENTS.md
//! § Testing against tmux). The only test in this binary: it sets process env vars.

use cdamux_core::{
    restore_one, session_registry, spawn_tmux_session_with_layout, AgentEntry, AgentsConfig,
    PaneLayout, RestoreOutcome,
};
use std::path::Path;
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

        let after = session_registry::load(name).unwrap();
        assert_eq!(after.agent_session_id.as_deref(), Some(id.as_str()));
        assert_eq!(after.created_ts, before.created_ts);
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
