//! `kill_tmux_session` must not take down the server when
//! `detach-on-destroy` is `next` (tmux 3.7.x bug).

use cdamux_core::{env_lock, kill_tmux_session, spawn_tmux_session_with_layout, PaneLayout};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-kill-detach-test";

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn tmux(args: &[&str]) -> std::process::Output {
    Command::new("tmux").args(args).output().expect("tmux")
}

#[test]
fn kill_with_detach_next_keeps_server() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };

    let tmp = tempfile::tempdir().unwrap();
    let conf = tmp.path().join("tmux.conf");
    std::fs::write(
        &conf,
        "set -g destroy-unattached off\nset -g detach-on-destroy next\n",
    )
    .unwrap();

    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f {} \"$@\"\n",
            conf.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::remove_var("TMUX");

    assert!(
        tmux(&["new-session", "-d", "-s", "keep", "-c", "/tmp", "--", "sleep", "60"])
            .status
            .success()
    );
    assert!(
        tmux(&[
            "new-session",
            "-d",
            "-s",
            "victim",
            "-c",
            "/tmp",
            "--",
            "sleep",
            "60"
        ])
        .status
        .success()
    );

    kill_tmux_session("victim").expect("kill victim");

    let start = Instant::now();
    let alive = loop {
        let ok = tmux(&["list-sessions"]).status.success();
        if ok || start.elapsed() > Duration::from_secs(2) {
            break ok;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(alive, "server died after kill_tmux_session with detach-on-destroy next");

    let listed = tmux(&["list-sessions", "-F", "#{session_name}"]);
    let names = String::from_utf8_lossy(&listed.stdout);
    assert!(names.contains("keep"), "keep gone: {names}");
    assert!(!names.contains("victim"), "victim still live: {names}");

    let sock_out = tmux(&["display-message", "-p", "#{socket_path}"]);
    let sock = String::from_utf8_lossy(&sock_out.stdout);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = tmux(&["kill-server"]);
}

/// Host configs (nix-darwin `/etc/tmux.conf`) set `next` on every server start, and
/// agents run a bare `tmux kill-session` on finished workers: spawning a worker must
/// already make that kill local to the one session.
#[test]
fn bare_kill_of_spawned_worker_keeps_server() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let _env = env_lock();
    let tmp = tempfile::tempdir().unwrap();
    let conf = tmp.path().join("tmux.conf");
    std::fs::write(
        &conf,
        "set -g destroy-unattached off\nset -g detach-on-destroy next\n",
    )
    .unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec {real_tmux} -L cdamux-bare-kill -f {} \"$@\"\n",
            conf.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("XDG_STATE_HOME", tmp.path().join("state"));
    std::env::remove_var("CDAMUX_HOME");
    std::env::set_var("SHELL", "/bin/sh");
    std::env::remove_var("TMUX");

    // Orchestrator stand-in, then two workers spawned through cdamux.
    assert!(
        tmux(&["new-session", "-d", "-s", "c-global", "-c", "/tmp", "--", "sleep", "60"])
            .status
            .success()
    );
    for name in ["c-done-worker", "c-live-worker"] {
        spawn_tmux_session_with_layout(name, tmp.path(), "sleep 60", PaneLayout::Single, false, None, Some("c-global"))
            .expect("spawn");
    }
    let opt = tmux(&["show-options", "-gv", "detach-on-destroy"]);
    assert_eq!(String::from_utf8_lossy(&opt.stdout).trim(), "on");

    // What the maintainer ran: a bare kill, not cdamux's kill path.
    let _ = tmux(&["kill-session", "-t", "c-done-worker"]);

    std::thread::sleep(Duration::from_millis(300));
    let listed = tmux(&["list-sessions", "-F", "#{session_name}"]);
    assert!(listed.status.success(), "server died after bare kill-session");
    let names = String::from_utf8_lossy(&listed.stdout);
    assert!(names.contains("c-global"), "orchestrator gone: {names}");
    assert!(names.contains("c-live-worker"), "other worker gone: {names}");
    assert!(!names.contains("c-done-worker"), "victim still live: {names}");

    let sock_out = tmux(&["display-message", "-p", "#{socket_path}"]);
    let sock = String::from_utf8_lossy(&sock_out.stdout);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = tmux(&["kill-server"]);
}
