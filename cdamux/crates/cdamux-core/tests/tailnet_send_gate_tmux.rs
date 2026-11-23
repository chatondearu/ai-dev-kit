//! Tailnet `send` on a throwaway tmux server (cdamux/AGENTS.md § Testing against tmux):
//! only sessions carrying `@cdamux_id` take remote input; a plain shell gets 403 and
//! nothing is typed into it (H4).

use cdamux_core::{
    dispatch_tailnet_request, env_lock, LiveTailnetBackend, ProjectsConfig, SessionsConfig,
};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-tailnet-gate-test";

fn which(cmd: &str) -> Option<String> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|d| Path::new(d).join(cmd))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

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

#[test]
fn send_only_reaches_sessions_with_a_cdamux_id() {
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

    tmux(&["new-session", "-d", "-s", "plain", "--", "/bin/sh"]);
    tmux(&["new-session", "-d", "-s", "agent", "--", "/bin/sh"]);
    tmux(&["set-option", "-t", "=agent:", "@cdamux_id", "a-gate1"]);
    let socket = tmux(&["display", "-p", "#{socket_path}"]);
    assert!(Path::new(socket.trim()).starts_with(&root), "not a throwaway server: {socket}");

    let result = std::panic::catch_unwind(|| {
        let backend = LiveTailnetBackend::new(
            "test".into(),
            ProjectsConfig::default(),
            SessionsConfig::default(),
            60,
            600,
        );
        let send = |session: &str, file: &Path| {
            let body = format!(r#"{{"line":"touch {}","enter":true}}"#, file.display());
            dispatch_tailnet_request("POST", &format!("/v1/sessions/{session}/send"), &body, &backend)
        };
        let pwned = root.join("PWNED");
        let resp = send("plain", &pwned);
        assert_eq!(resp.status, 403, "{}", resp.body);
        assert!(resp.body.contains("\"code\":\"forbidden\""), "{}", resp.body);

        let ok = root.join("OK");
        let resp = send("agent", &ok);
        assert_eq!(resp.status, 200, "{}", resp.body);
        let start = Instant::now();
        while !ok.exists() {
            assert!(start.elapsed() < Duration::from_secs(10), "agent session never ran the line");
            std::thread::sleep(Duration::from_millis(100));
        }
        std::thread::sleep(Duration::from_millis(300));
        assert!(!pwned.exists(), "plain shell received remote input");
    });
    tmux(&["kill-server"]);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
