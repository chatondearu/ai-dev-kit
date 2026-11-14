//! `cdamux sidebar` end to end on a throwaway tmux server (cdamux/AGENTS.md § Testing
//! against tmux): the cursor moves one client across sessions, the other client keeps
//! its view, Enter hands focus to the session pane, `q` closes the sidebar.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-sidebar-e2e";

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
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}\n{}", dump());
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

fn sidebar_pane(client: &str) -> Option<String> {
    tmux(&["list-panes", "-a", "-F", "#{pane_id}\t#{@cdamux_sidebar}"])
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(_, owner)| *owner == client)
        .map(|(p, _)| p.to_string())
}

fn active_pane(session: &str) -> String {
    tmux(&["display-message", "-p", "-t", &format!("={session}:"), "#{pane_id}"])
}

#[test]
fn cursor_moves_one_client_and_enter_focuses_the_session() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n")).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()));
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("config"));
    std::env::set_var("XDG_STATE_HOME", tmp.path().join("state"));
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::set_var("SHELL", "/bin/sh");
    std::env::remove_var("TMUX");
    std::env::remove_var("TMUX_PANE");

    for s in ["a", "b", "c"] {
        let status = Command::new("tmux")
            .args(["new-session", "-d", "-x", "120", "-y", "40", "-s", s, "-c"])
            .arg(tmp.path())
            .arg("sleep 600")
            .status()
            .unwrap();
        assert!(status.success());
    }
    let mut clients = vec![control_client("a"), control_client("c")];

    let cdamux = |client: &str| {
        Command::new(env!("CARGO_BIN_EXE_cdamux"))
            .args(["sidebar", "--left", "--width", "30", "--client", client])
            .env("TMUX", "throwaway")
            .status()
            .unwrap()
    };

    let result = std::panic::catch_unwind(|| {
        wait_until("both clients", || client_on("a").is_some() && client_on("c").is_some(), String::new);
        let (c1, c2) = (client_on("a").unwrap(), client_on("c").unwrap());

        assert!(cdamux(&c1).success());
        let bar = sidebar_pane(&c1).expect("sidebar pane");
        let dump = || tmux(&["capture-pane", "-p", "-t", &bar]);
        wait_until("tree drawn", || dump().contains("unassigned"), dump);

        // The cursor starts on a; j rests on b → c1 switches, the sidebar comes along.
        tmux(&["send-keys", "-t", &bar, "j"]);
        wait_until("c1 on b", || session_of(&c1) == "b", dump);
        assert_eq!(session_of(&c2), "c", "second client keeps its view");
        assert_eq!(tmux(&["display-message", "-p", "-t", &bar, "#{session_name}"]), "b");

        tmux(&["send-keys", "-t", &bar, "j"]);
        wait_until("c1 on c", || session_of(&c1) == "c", dump);
        assert_eq!(session_of(&c2), "c");

        // Enter hands focus to the session pane; the launcher refocuses the sidebar.
        tmux(&["send-keys", "-t", &bar, "Enter"]);
        wait_until("main pane focused", || active_pane("c") != bar, dump);
        assert!(cdamux(&c1).success());
        assert_eq!(sidebar_pane(&c1).as_deref(), Some(bar.as_str()), "one sidebar per client");
        assert_eq!(active_pane("c"), bar);

        tmux(&["send-keys", "-t", &bar, "q"]);
        wait_until("sidebar closed", || sidebar_pane(&c1).is_none(), || tmux(&["list-panes", "-a"]));
        assert_eq!(session_of(&c1), "c");
    });

    for c in &mut clients {
        let _ = c.kill();
    }
    // Kill only our private server, after checking its socket lives under tmp.
    let socket = tmux(&["display", "-p", "-t", "=a", "#{socket_path}"]);
    let tmp_real = tmp.path().canonicalize().unwrap();
    let socket = Path::new(socket.trim()).canonicalize().unwrap_or_default();
    if socket.starts_with(&tmp_real) && !socket.to_string_lossy().contains("/default") {
        tmux(&["kill-server"]);
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
