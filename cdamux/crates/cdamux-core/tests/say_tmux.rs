//! `say` paste / queue / drain on a throwaway tmux server (cdamux/AGENTS.md): the pane
//! runs `cat`, so whatever is pasted shows up in `capture-pane`.

use cdamux_core::{env_lock, say, say_drain, state_append, state_set, SayOutcome, StateKind};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-say-test";

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn pane(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-J", "-t", &format!("={session}:")])
        .output()
        .expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn wait_for_pane(session: &str, needle: &str) -> String {
    let start = Instant::now();
    loop {
        let text = pane(session);
        if text.contains(needle) || start.elapsed() > Duration::from_secs(5) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn set_status(id: &str, status: &str) {
    state_append(StateKind::Agents, id, &serde_json::json!({ "status": status })).unwrap();
}

#[test]
fn say_pastes_when_idle_queues_when_busy_and_drains_in_order() {
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
    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");

    assert!(Command::new("tmux")
        .args(["new-session", "-d", "-s", "c-say", "cat"])
        .status()
        .unwrap()
        .success());
    state_set(
        StateKind::Agents,
        "a-osay1",
        &serde_json::json!({
            "id": "a-osay1", "client": "claude", "host": "c-say",
            "role": "orchestrator", "status": "idle"
        }),
    )
    .unwrap();

    // Idle + quiet pane (injected): pasted at once, prefixed with the label.
    let t0 = Instant::now();
    let r = say("a-osay1", "alice", "hello-idle", 10, Some(Some(15))).unwrap();
    assert_eq!(r.outcome, SayOutcome::Delivered);
    let text = wait_for_pane("c-say", "[alice] hello-idle");
    assert!(text.contains("[alice] hello-idle"), "no paste: {text}");
    assert!(t0.elapsed() < Duration::from_secs(2), "delivery took {:?}", t0.elapsed());

    // Mid-turn: queued, nothing pasted, and the drain refuses to run a working turn.
    set_status("a-osay1", "working");
    let a = say("a-osay1", "bob", "first-queued", 10, Some(Some(15))).unwrap();
    let b = say("a-osay1", "carol", "second-queued", 10, Some(Some(15))).unwrap();
    assert_eq!((a.outcome, b.outcome), (SayOutcome::Queued, SayOutcome::Queued));
    assert_eq!(say_drain("a-osay1").unwrap(), None);
    std::thread::sleep(Duration::from_millis(300));
    let text = pane("c-say");
    assert!(!text.contains("queued"), "busy orchestrator got a paste: {text}");

    // Turn end 1: exactly one prompt, the oldest. Turn end 2: the next one.
    set_status("a-osay1", "idle");
    assert_eq!(say_drain("a-osay1").unwrap(), Some(a.id));
    let text = wait_for_pane("c-say", "[bob] first-queued");
    assert!(text.contains("[bob] first-queued") && !text.contains("second-queued"), "{text}");
    set_status("a-osay1", "working");
    set_status("a-osay1", "idle");
    assert_eq!(say_drain("a-osay1").unwrap(), Some(b.id));
    let text = wait_for_pane("c-say", "[carol] second-queued");
    assert!(text.contains("[carol] second-queued"), "{text}");
    assert_eq!(say_drain("a-osay1").unwrap(), None);

    let sock = Command::new("tmux")
        .args(["display-message", "-p", "#{socket_path}"])
        .output()
        .unwrap();
    let sock = String::from_utf8_lossy(&sock.stdout).into_owned();
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}
