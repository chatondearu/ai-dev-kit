//! `ensure_home_session` respawns a home pane restored as a plain shell
//! (resurrect / continuum) and leaves any other foreground command alone.

use cdamux_core::{ensure_home_session, env_lock};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-home-repair-test";

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

fn home_pane(format: &str) -> String {
    let out = tmux(&["display-message", "-p", "-t", "=cdamux:", format]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn wait_home_command(want: &str) -> bool {
    let start = Instant::now();
    loop {
        if home_pane("#{pane_current_command}") == want {
            return true;
        }
        if start.elapsed() > Duration::from_secs(3) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn home_shell_pane_is_respawned_other_commands_kept() {
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
    // Fake dashboard. tmux reads `pane_current_command` from argv[0] (`/proc/<pgrp>/cmdline`
    // on Linux), so a plain `#!/bin/sh` script would show up as `sh`. bash, not sleep, takes
    // the renamed argv[0]: a multicall coreutils rejects `cdamux`; `; :` keeps bash alive.
    let fake = bin.join("cdamux");
    std::fs::write(&fake, "#!/usr/bin/env bash\nexec -a cdamux bash -c 'sleep 30; :'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    for f in [&wrapper, &fake] {
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::remove_var("TMUX");

    // Keeps the server up while the home session is recreated between cases.
    assert!(
        tmux(&["new-session", "-d", "-s", "keep", "-c", "/tmp", "--", "sleep", "30"])
            .status
            .success()
    );

    assert!(
        tmux(&["new-session", "-d", "-s", "cdamux", "-c", "/tmp", "--", "sh"])
            .status
            .success()
    );
    assert!(wait_home_command("sh"), "home pane never ran sh");
    ensure_home_session().expect("ensure home (shell)");
    assert!(
        wait_home_command("cdamux"),
        "shell home pane not respawned: {}",
        home_pane("#{pane_current_command}")
    );

    assert!(tmux(&["kill-session", "-t", "=cdamux"]).status.success());
    assert!(
        tmux(&["new-session", "-d", "-s", "cdamux", "-c", "/tmp", "--", "sleep", "30"])
            .status
            .success()
    );
    assert!(wait_home_command("sleep"), "home pane never ran sleep");
    let pid = home_pane("#{pane_pid}");
    ensure_home_session().expect("ensure home (busy)");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(home_pane("#{pane_current_command}"), "sleep");
    assert_eq!(home_pane("#{pane_pid}"), pid, "busy home pane was respawned");

    let sock_out = tmux(&["display-message", "-p", "#{socket_path}"]);
    let sock = String::from_utf8_lossy(&sock_out.stdout);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = tmux(&["kill-server"]);
}
