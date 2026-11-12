//! Worker paste + orch idle nudge on a throwaway tmux server (cdamux/AGENTS.md).

use cdamux_core::{
    msg_send, state_set, DeliveryMode, StateKind, DEFAULT_NUDGE_QUIET_SECS,
};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-msg-test";

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn wait_for_pane(session: &str, needle: &str) -> String {
    let start = Instant::now();
    loop {
        let text = tmux(&[
            "capture-pane",
            "-p",
            "-J",
            "-t",
            &format!("={session}:"),
        ]);
        if text.contains(needle) || start.elapsed() > Duration::from_secs(5) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn worker_paste_and_orch_nudge() {
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
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");

    // Worker pane: sleep so send-keys land in a live pane.
    assert!(Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            "c-worker",
            "sh",
            "-c",
            "sleep 30",
        ])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            "c-orch",
            "sh",
            "-c",
            "sleep 30",
        ])
        .status()
        .unwrap()
        .success());

    state_set(
        StateKind::Agents,
        "a-wmsg1",
        &serde_json::json!({
            "id": "a-wmsg1",
            "client": "claude",
            "host": "c-worker",
            "role": "worker",
            "parent": "a-omsg1",
            "status": "working"
        }),
    )
    .unwrap();
    state_set(
        StateKind::Agents,
        "a-omsg1",
        &serde_json::json!({
            "id": "a-omsg1",
            "client": "claude",
            "host": "c-orch",
            "role": "orchestrator",
            "status": "idle",
            "parent": null
        }),
    )
    .unwrap();

    let pasted = msg_send(
        "a-omsg1",
        "a-wmsg1",
        "relay",
        "hello-worker-msg",
        None,
        DEFAULT_NUDGE_QUIET_SECS,
        None,
    )
    .unwrap();
    assert_eq!(pasted.delivery, DeliveryMode::WorkerPaste);
    assert!(pasted.pasted);
    let pane = wait_for_pane("c-worker", "hello-worker-msg");
    assert!(
        pane.contains("hello-worker-msg"),
        "worker pane missing paste: {pane}"
    );

    // Orch nudge: inject quiet window so we do not depend on window_activity age.
    let nudged = msg_send(
        "a-wmsg1",
        "a-omsg1",
        "done",
        "pr opened",
        None,
        10,
        Some(Some(15)),
    )
    .unwrap();
    assert_eq!(nudged.delivery, DeliveryMode::OrchNudge);
    assert!(nudged.nudged);
    let orch = wait_for_pane("c-orch", "cdamux msg read");
    assert!(
        orch.contains("cdamux msg read"),
        "orch pane missing nudge: {orch}"
    );

    let sock = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}
