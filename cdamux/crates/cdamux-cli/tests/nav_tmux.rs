//! `cdamux nav` on a throwaway tmux server: opening the navigator does not add
//! panes to an agent session; switch-client reaches the nav session.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-nav-e2e";

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

fn which(cmd: &str) -> Option<String> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|d| Path::new(d).join(cmd))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool, dump: impl Fn() -> String) {
    let start = Instant::now();
    while !ok() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}\n{}",
            dump()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn control_client(session: &str) -> Child {
    Command::new("tmux")
        .args(["-C", "attach-session", "-t", session])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("control client")
}

fn client_on(session: &str) -> Option<String> {
    tmux(&["list-clients", "-F", "#{client_name}\t#{session_name}"])
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(_, s)| *s == session)
        .map(|(c, _)| c.to_string())
}

fn session_of(client: &str) -> String {
    tmux(&["list-clients", "-F", "#{client_name}\t#{session_name}"])
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(c, _)| *c == client)
        .map(|(_, s)| s.to_string())
        .unwrap_or_default()
}

fn pane_count(session: &str) -> usize {
    tmux(&["list-panes", "-t", &format!("={session}"), "-F", "#{pane_id}"])
        .lines()
        .filter(|l| !l.is_empty())
        .count()
}

#[test]
fn nav_session_leaves_agent_pane_count_unchanged() {
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
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Put the built cdamux first so ensure_nav_session's pane command finds it.
    let cdamux_bin = Path::new(env!("CARGO_BIN_EXE_cdamux"));
    let cdamux_dir = cdamux_bin.parent().unwrap();
    std::env::set_var(
        "PATH",
        format!(
            "{}:{}:{}",
            cdamux_dir.display(),
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    std::env::set_var("XDG_STATE_HOME", tmp.path().join("state"));
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::set_var("SHELL", "/bin/sh");
    std::env::remove_var("TMUX");
    std::env::remove_var("TMUX_PANE");

    for s in ["agent", "other"] {
        let status = Command::new("tmux")
            .args(["new-session", "-d", "-x", "120", "-y", "40", "-s", s, "-c"])
            .arg(tmp.path())
            .arg("sleep 600")
            .status()
            .unwrap();
        assert!(status.success());
    }
    let mut clients = vec![control_client("agent")];

    let result = std::panic::catch_unwind(|| {
        wait_until(
            "client on agent",
            || client_on("agent").is_some(),
            String::new,
        );
        let c1 = client_on("agent").unwrap();
        assert_eq!(pane_count("agent"), 1, "agent starts with one pane");

        let status = Command::new(env!("CARGO_BIN_EXE_cdamux"))
            .args(["nav", "--client", &c1])
            .env("TMUX", "throwaway")
            .status()
            .unwrap();
        assert!(status.success(), "cdamux nav");

        wait_until(
            "on cdamux-nav",
            || session_of(&c1) == "cdamux-nav",
            || tmux(&["list-sessions", "-F", "#{session_name}"]),
        );
        assert_eq!(
            pane_count("agent"),
            1,
            "opening nav must not split the agent session"
        );
        assert_eq!(pane_count("cdamux-nav"), 1);

        // Toggle back.
        let status = Command::new(env!("CARGO_BIN_EXE_cdamux"))
            .args(["nav", "--client", &c1])
            .env("TMUX", "throwaway")
            .status()
            .unwrap();
        assert!(status.success());
        wait_until(
            "back on agent",
            || session_of(&c1) == "agent",
            || format!("session={}", session_of(&c1)),
        );
        assert_eq!(pane_count("agent"), 1);
        assert!(
            Command::new("tmux")
                .args(["has-session", "-t", "=cdamux-nav"])
                .status()
                .unwrap()
                .success(),
            "nav session stays alive after toggle away"
        );
    });

    for c in &mut clients {
        let _ = c.kill();
    }
    let socket = tmux(&["display", "-p", "-t", "=agent", "#{socket_path}"]);
    let tmp_real = tmp.path().canonicalize().unwrap();
    let socket = Path::new(socket.trim()).canonicalize().unwrap_or_default();
    if socket.starts_with(&tmp_real) && !socket.to_string_lossy().contains("/default") {
        tmux(&["kill-server"]);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
