//! Sidebar pane follows one client across three sessions on a throwaway tmux server
//! (cdamux/AGENTS.md § Testing against tmux), with a second client left untouched.

use cdamux_core::{
    client_session, env_lock, find_sidebar, focus_main_pane, follow_session, spawn_sidebar,
    SidebarSide,
};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

const SOCKET: &str = "cdamux-sidebar-test";

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

/// `tmux -C attach`: a client without a terminal, named like any other.
fn control_client(session: &str) -> Child {
    Command::new("tmux")
        .args(["-C", "attach-session", "-t", session])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("control client")
}

fn client_of(session: &str) -> String {
    for _ in 0..50 {
        let out = tmux(&["list-clients", "-F", "#{client_name}\t#{session_name}"]);
        if let Some((name, _)) = out.lines().filter_map(|l| l.split_once('\t')).find(|(_, s)| *s == session) {
            return name.to_string();
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("no client on {session}");
}

fn pane_count(session: &str) -> usize {
    tmux(&["list-panes", "-t", &format!("={session}:"), "-F", "#{pane_id}"]).lines().count()
}

fn active_pane(session: &str) -> String {
    tmux(&["display-message", "-p", "-t", &format!("={session}:"), "#{pane_id}"])
}

#[test]
fn sidebar_follows_one_client_and_leaves_the_other() {
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

    let _env = env_lock();
    std::env::set_var("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()));
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::remove_var("TMUX");
    std::env::remove_var("TMUX_PANE");

    for s in ["a", "b", "c"] {
        assert!(Command::new("tmux")
            .args(["new-session", "-d", "-x", "120", "-y", "40", "-s", s, "sleep 600"])
            .status()
            .unwrap()
            .success());
    }
    let mut clients = vec![control_client("a"), control_client("c")];

    let result = std::panic::catch_unwind(|| {
        let (c1, c2) = (client_of("a"), client_of("c"));
        assert_eq!(client_session(&c1).unwrap(), "a");

        let sleep = ["sleep".to_string(), "600".to_string()];
        let bar = spawn_sidebar(&c1, SidebarSide::Left, 30, &sleep).unwrap();
        assert_eq!(find_sidebar(&c1).as_deref(), Some(bar.as_str()));
        assert_eq!(find_sidebar(&c2), None);
        assert_eq!(pane_count("a"), 2);
        assert_eq!(tmux(&["display-message", "-p", "-t", &bar, "#{pane_width}"]), "30");

        // Cursor-driven: c1 moves to b; the sidebar comes along and keeps focus.
        follow_session(&c1, &bar, "b", SidebarSide::Left, 30, true).unwrap();
        assert_eq!(client_session(&c1).unwrap(), "b");
        assert_eq!(client_session(&c2).unwrap(), "c", "second client keeps its view");
        assert_eq!((pane_count("a"), pane_count("b"), pane_count("c")), (1, 2, 1));
        assert_eq!(active_pane("b"), bar);
        assert_eq!(tmux(&["display-message", "-p", "-t", &bar, "#{pane_width}"]), "30");

        // Enter: focus the pane the sidebar sits next to.
        focus_main_pane(&bar).unwrap();
        assert_ne!(active_pane("b"), bar);

        // Into the session the other client shows: its window gets the pane, its view stays.
        follow_session(&c1, &bar, "c", SidebarSide::Left, 30, true).unwrap();
        assert_eq!(client_session(&c1).unwrap(), "c");
        assert_eq!(client_session(&c2).unwrap(), "c");
        assert_eq!((pane_count("b"), pane_count("c")), (1, 2));

        // Same session again is a no-op; following without focus keeps the main pane active.
        follow_session(&c1, &bar, "c", SidebarSide::Left, 30, true).unwrap();
        assert_eq!(pane_count("c"), 2);
        follow_session(&c1, &bar, "a", SidebarSide::Left, 30, false).unwrap();
        assert_eq!(client_session(&c1).unwrap(), "a");
        assert_ne!(active_pane("a"), bar, "external follow does not steal focus");
        assert_eq!(client_session(&c2).unwrap(), "c");
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
