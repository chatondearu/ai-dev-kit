//! `list_orchestrators` (`cdamux orch ls`) and read-only `capture_session` (#388)
//! on a throwaway tmux server + temp CDAMUX_HOME.

use cdamux_core::{
    capture_session, env_lock, find_live_global_orchestrator, list_orchestrators, state_set,
    CaptureError, StateKind,
};
use serde_json::json;
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdx-orchls";

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Restores the process env vars a test mutates, even when it panics.
struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl EnvRestore {
    fn save(keys: &[&'static str]) -> Self {
        Self(keys.iter().map(|k| (*k, std::env::var_os(k))).collect())
    }
}

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

fn tmux(args: &[&str]) -> std::process::Output {
    Command::new("tmux").args(args).output().expect("tmux")
}

#[test]
fn orch_ls_and_capture_on_throwaway_server() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };

    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let argv_log = tmp.path().join("tmux-argv.log");
    // Log every tmux subcommand so the test can prove capture never sends input.
    std::fs::write(
        bin.join("tmux"),
        format!(
            "#!/bin/sh\necho \"$*\" >> {}\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n",
            argv_log.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("tmux"), std::fs::Permissions::from_mode(0o755)).unwrap();

    let _env = env_lock();
    let _restore = EnvRestore::save(&["PATH", "TMUX_TMPDIR", "TMUX", "CDAMUX_HOME"]);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");

    for (name, cmd) in [
        ("c-global", "sleep 30"),
        ("c-proj", "printf 'hello-capture\\n'; sleep 30"),
        // Hidden from `cdamux list`: only `--any` may capture it.
        ("cdamux-nav", "sleep 30"),
    ] {
        let out = tmux(&["new-session", "-d", "-s", name, cmd]);
        assert!(out.status.success(), "{name}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let sock = String::from_utf8_lossy(&tmux(&["display-message", "-p", "#{socket_path}"]).stdout)
        .into_owned();
    assert!(sock.contains(tmp.path().to_str().unwrap()), "socket {sock} not under temp");

    let agents = [
        ("a-glob", json!({"id": "a-glob", "role": "orchestrator", "scope": "global", "host": "c-global", "status": "idle", "parent": null, "run": "r-g"})),
        ("a-proj", json!({"id": "a-proj", "role": "orchestrator", "host": "c-proj", "parent": "a-glob", "run": "r-p"})),
        ("a-stale", json!({"id": "a-stale", "role": "orchestrator", "host": "", "status": "working"})),
        ("a-dead", json!({"id": "a-dead", "role": "orchestrator", "scope": "global", "host": "gone"})),
        ("a-work", json!({"id": "a-work", "role": "worker", "host": "c-proj", "parent": "a-proj"})),
    ];
    for (id, v) in &agents {
        state_set(StateKind::Agents, id, v).unwrap();
    }
    state_set(StateKind::Runs, "r-p", &json!({"id": "r-p", "parent": "a-proj", "repo": "/src/proj"})).unwrap();
    state_set(StateKind::Runs, "r-g", &json!({"id": "r-g", "parent": "a-glob", "repo": "/home/global"})).unwrap();

    assert_eq!(find_live_global_orchestrator(), Some(("a-glob".into(), "c-global".into())));

    let got = serde_json::to_value(list_orchestrators()).unwrap();
    assert_eq!(
        got,
        json!([
            {"id": "a-dead", "host": "gone", "scope": "global", "status": null, "live": false},
            {"id": "a-glob", "host": "c-global", "scope": "global", "status": "idle", "live": true},
            {"id": "a-proj", "host": "c-proj", "scope": "project", "repo": "/src/proj", "status": null, "live": true, "parent": "a-glob"},
            {"id": "a-stale", "host": "", "scope": "project", "status": "working", "live": false},
        ])
    );

    // Capture: only read-only tmux subcommands from here on.
    std::fs::write(&argv_log, "").unwrap();
    let start = Instant::now();
    let cap = loop {
        let cap = capture_session("c-proj", 10, false).expect("capture listed session");
        if cap.text.contains("hello-capture") || start.elapsed() > Duration::from_secs(5) {
            break cap;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(cap.text.contains("hello-capture"), "{cap:?}");
    assert_eq!((cap.v, cap.session.as_str(), cap.lines), (1, "c-proj", 10));
    assert!(cap.pane.starts_with('%'), "pane={}", cap.pane);

    assert_eq!(capture_session("c-proj", 0, false).unwrap().lines, 1);
    assert_eq!(capture_session("c-proj", 9999, false).unwrap().lines, 500);
    assert!(matches!(capture_session("nope", 10, false), Err(CaptureError::Unknown(_))));
    assert!(matches!(capture_session("nope", 10, true), Err(CaptureError::Unknown(_))));
    // Exact match only: `c-pro` must not prefix-match `c-proj`.
    assert!(matches!(capture_session("c-pro", 10, true), Err(CaptureError::Unknown(_))));
    assert!(matches!(capture_session("cdamux-nav", 10, false), Err(CaptureError::Unknown(_))));
    assert!(capture_session("cdamux-nav", 10, true).is_ok());

    let log = std::fs::read_to_string(&argv_log).unwrap();
    for line in log.lines() {
        let sub = line.split_whitespace().next().unwrap_or_default();
        assert!(
            ["list-sessions", "has-session", "display-message", "capture-pane"].contains(&sub),
            "capture ran a non-read-only tmux command: {line}"
        );
    }

    let _ = Command::new("tmux").args(["kill-server"]).status();
}
