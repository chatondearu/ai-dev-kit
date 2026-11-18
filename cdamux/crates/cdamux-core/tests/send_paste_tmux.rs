//! Text sent to a pane on a throwaway tmux server (cdamux/AGENTS.md § Testing against
//! tmux): a `\r` inside the text never submits it, and no paste buffer (which may hold a
//! secret) is left behind after a send, whether it succeeded or failed.

use cdamux_core::{env_lock, send_line_to_session, send_to_session};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-send-paste-test";

fn which(cmd: &str) -> Option<String> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|d| Path::new(d).join(cmd))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

/// Restores the process env vars a test mutates, even when it panics.
struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (k, v) in &self.0 {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}:\n{}", tmux(&["capture-pane", "-p", "-t", "=t1:"]));
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn carriage_return_does_not_submit_and_no_buffer_is_left() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();

    let _env = env_lock();
    let _restore = EnvRestore(["PATH", "TMUX_TMPDIR", "TMUX"].iter().map(|k| (*k, std::env::var_os(k))).collect());
    std::env::set_var("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()));
    std::env::set_var("TMUX_TMPDIR", &root);
    std::env::remove_var("TMUX");

    tmux(&["new-session", "-d", "-s", "t1", "-x", "160", "-y", "30", "--", "/bin/sh"]);
    let socket = tmux(&["display", "-p", "#{socket_path}"]);
    assert!(Path::new(socket.trim()).starts_with(&root), "not a throwaway server: {socket}");

    let result = std::panic::catch_unwind(|| {
        let pwned = root.join("PWNED");
        let ok = root.join("OK");

        // `\r` would submit the line in a pane without bracketed paste: it must be dropped.
        send_to_session("t1", &format!("touch {}\r", pwned.display()), false).expect("send");
        wait_until("typed text", || tmux(&["capture-pane", "-p", "-t", "=t1:"]).contains("PWNED"));
        std::thread::sleep(Duration::from_millis(500));
        assert!(!pwned.exists(), "\\r in text executed the command");
        assert_eq!(tmux(&["list-buffers"]), "", "buffer left after a successful paste");

        // Same line, cleared, then submitted with an explicit Enter: the paste path works.
        tmux(&["send-keys", "-t", "=t1:", "C-c"]);
        send_to_session("t1", &format!("touch {}", ok.display()), true).expect("send");
        wait_until("Enter to submit", || ok.exists());
        assert!(!pwned.exists());

        // Paste to a missing target fails after load-buffer: the text must not linger.
        assert!(send_line_to_session("no-such-session", "hunter2", false).is_err());
        assert_eq!(tmux(&["list-buffers"]), "", "buffer left after a failed paste");
    });
    tmux(&["kill-server"]);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
