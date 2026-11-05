//! `cdamux restart --all` typed inside the session it restarts, on a throwaway tmux
//! server (cdamux/AGENTS.md § Testing against tmux): the busy session is skipped, the
//! idle one restarted, and the current session restarted last by the detached child
//! after its own pane (and the CLI in it) is gone.

use cdamux_core::{spawn_tmux_session_with_layout, PaneLayout};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-restart-test";

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn panes(session: &str) -> Vec<String> {
    tmux(&["list-panes", "-t", &format!("={session}:"), "-F", "#{pane_id}"])
        .lines()
        .map(str::to_string)
        .collect()
}

fn pane_text(session: &str) -> String {
    panes(session).iter().map(|p| tmux(&["capture-pane", "-p", "-J", "-t", p])).collect()
}

/// Poll `ok` for up to 10 s; on timeout panic with `what` and the `dump()` diagnostics.
fn wait_until(what: &str, mut ok: impl FnMut() -> bool, dump: impl Fn() -> String) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}\n{}", dump());
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn restart_all_skips_busy_and_restarts_current_session_last() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    // Wrapper so every tmux call (ours, the CLI's, the detached child's) hits a private `-L` server.
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()));
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_STATE_HOME", tmp.path().join("state"));
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    std::env::set_var("SHELL", "/bin/sh");
    std::env::remove_var("TMUX");
    std::env::remove_var("TMUX_PANE");
    // `sleep` stands in for claude; `#` turns the appended flags into a visible comment.
    let cfg = tmp.path().join("config/cdamux");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(
        cfg.join("config.toml"),
        "schema_version = 1\n[agents]\ndefault = \"claude\"\n[agents.claude]\ncommand = \"sleep 600 #\"\nresume = \"sleep 600 # resumed {id}\"\n",
    )
    .unwrap();

    let cwd = tmp.path();
    for (name, layout) in [("a-cur", PaneLayout::Horizontal), ("b-busy", PaneLayout::Single), ("c-idle", PaneLayout::Single)] {
        spawn_tmux_session_with_layout(name, cwd, "sleep 600 #", layout, false, Some("claude"), None).expect("spawn");
    }

    let result = std::panic::catch_unwind(|| {
        tmux(&["set-option", "-t", "b-busy", "@cdamux_state", "running"]);
        let before: Vec<String> = ["a-cur", "b-busy", "c-idle"].iter().map(|s| panes(s)[0].clone()).collect();
        let out = tmp.path().join("out.txt");
        // The second pane of a-cur is a shell: run the CLI there, so a-cur is the current session.
        let shell = panes("a-cur")[1].clone();
        let cmd = format!("{} restart --all > {} 2>&1", env!("CARGO_BIN_EXE_cdamux"), out.display());
        tmux(&["send-keys", "-t", &shell, &cmd, "Enter"]);

        let log_path = tmp.path().join("state/cdamux/restart.log");
        let dump = || {
            format!(
                "out: {:?}\nlog: {:?}\nsessions: {}\na-cur: {}",
                std::fs::read_to_string(&out),
                std::fs::read_to_string(&log_path),
                tmux(&["list-sessions", "-F", "#{session_name}"]),
                pane_text("a-cur")
            )
        };
        wait_until("a-cur restarted", || panes("a-cur").first().is_some_and(|p| *p != before[0]), dump);
        // The detached child logs its line; "restored" = the resume command ran. (The
        // typed command is not read back from a-cur: its 40-column split pane mangles it.)
        let read_log = || std::fs::read_to_string(&log_path).unwrap_or_default();
        wait_until("a-cur restore logged", || read_log().contains("a-cur: restarted, restored"), dump);
        let lines = std::fs::read_to_string(&out).unwrap();
        assert!(lines.contains("b-busy: skipped (busy"), "{lines}");
        assert!(lines.contains("c-idle: restarted, restored"), "{lines}");
        assert!(lines.contains("a-cur: restarting in the background (current session)"), "{lines}");
        let a = lines.find("a-cur:").unwrap();
        assert!(lines.find("c-idle:").unwrap() < a, "current session last: {lines}");

        assert_eq!(panes("b-busy")[0], before[1], "busy session untouched");
        assert_ne!(panes("c-idle")[0], before[2]);
        wait_until("c-idle resumed", || pane_text("c-idle").contains("resumed"), dump);
    });

    // Kill only our private server, after checking its socket lives under tmp.
    let socket = tmux(&["display", "-p", "-t", "=b-busy", "#{socket_path}"]);
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
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|d| Path::new(d).join(cmd))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}
