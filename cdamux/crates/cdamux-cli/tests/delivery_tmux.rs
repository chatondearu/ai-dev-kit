//! Message delivery (ADR-018, Q21) end to end on a throwaway tmux server: the real
//! `cdamux` binary (its detached `msg wake` waker included), the real `hooks/status.js`
//! status hook, and tmux's own pane idle time — nothing injected. Only the wake window is
//! shortened (`CDAMUX_WAKE_QUIET_SECS=2` instead of 60 s) so the suite stays fast.
//!
//! Each fake agent pane runs a shell loop that appends every submitted line to a log file:
//! the log is what the agent would have received as a prompt.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-delivery-test";
const QUIET: u64 = 2;
const ORCH: &str = "a-dorch";

fn which(cmd: &str) -> Option<String> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|d| Path::new(d).join(cmd))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

struct Lab {
    tmp: tempfile::TempDir,
    path: String,
    status_js: PathBuf,
}

impl Lab {
    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    fn cmd(&self, program: &str) -> Command {
        let mut c = Command::new(program);
        c.env("PATH", &self.path)
            .env("TMUX_TMPDIR", self.tmp.path())
            .env("CDAMUX_HOME", self.home())
            .env("CDAMUX_WAKE_QUIET_SECS", QUIET.to_string())
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .env_remove("CDAMUX_AGENT_ID");
        c
    }

    fn tmux(&self, args: &[&str]) -> String {
        let out = self.cmd("tmux").args(args).output().expect("tmux");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn cdamux(&self, args: &[&str]) -> Output {
        self.cmd(env!("CARGO_BIN_EXE_cdamux")).args(args).output().expect("cdamux")
    }

    /// `msg send` from `from`; returns the delivery word.
    fn send(&self, from: &str, to: &str, kind: &str, text: &str) -> String {
        let out = self.cdamux(&["msg", "send", "--from", from, "--to", to, "--kind", kind, text]);
        assert!(out.status.success(), "msg send {kind}: {out:?}");
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        line.split_whitespace().nth(1).unwrap_or_default().to_string()
    }

    /// Run the status hook as agent `id` with one hook payload; returns its stdout.
    fn hook(&self, id: &str, payload: &str) -> String {
        use std::io::Write;
        let mut child = self
            .cmd("node")
            .arg(&self.status_js)
            .env("CDAMUX_AGENT_ID", id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("node");
        child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "hook {payload}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn record(&self, id: &str) -> Value {
        let text = std::fs::read_to_string(self.home().join(format!("agents/{id}.json")))
            .unwrap_or_else(|_| "{}".into());
        serde_json::from_str(&text).unwrap()
    }

    fn unread(&self, id: &str) -> usize {
        let out = self.cdamux(&["msg", "read", "--agent", id]);
        String::from_utf8_lossy(&out.stdout).lines().count()
    }

    fn log(&self, id: &str) -> String {
        std::fs::read_to_string(self.tmp.path().join(format!("{id}.log"))).unwrap_or_default()
    }

    /// Worker `id` hosted in tmux session `id` (see [`Lab::pane`]).
    fn worker(&self, id: &str, banner: &str) {
        self.pane(id, banner);
        self.record_worker(id);
    }

    /// Session `id`: a fake agent that logs submitted lines, after printing `banner`
    /// (e.g. a permission dialog).
    fn pane(&self, id: &str, banner: &str) {
        let log = self.tmp.path().join(format!("{id}.log"));
        let script = format!(
            "printf '%s\\n' '{banner}'; while IFS= read -r l; do printf '%s\\n' \"$l\" >> '{}'; done",
            log.display()
        );
        self.tmux(&["new-session", "-d", "-s", id, "-x", "160", "-y", "30", "sh", "-c", &script]);
    }

    fn record_worker(&self, id: &str) {
        let rec = format!(
            r#"{{"id":"{id}","client":"claude","host":"{id}","role":"worker","parent":"{ORCH}","status":"idle"}}"#
        );
        let out = self.cdamux(&["state", "append", "agents", id, &rec]);
        assert!(out.status.success(), "{out:?}");
    }
}

fn wait_until(what: &str, timeout_secs: u64, mut ok: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(timeout_secs), "timed out: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn quiet_windows(n: u64) {
    std::thread::sleep(Duration::from_secs(n * QUIET + 1));
}

#[test]
fn wake_line_only_never_mid_turn_draft_kept_and_submitted() {
    let (Some(real_tmux), Some(_)) = (which("tmux"), which("node")) else {
        eprintln!("tmux or node not on PATH: skipped");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n"))
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The hooks call `cdamux` from PATH: the binary under test.
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_cdamux"), bin.join("cdamux")).unwrap();
    let lab = Lab {
        path: format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()),
        status_js: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../hooks/status.js"),
        tmp,
    };
    let out = lab.cdamux(&["state", "append", "agents", ORCH, r#"{"id":"a-dorch","role":"orchestrator"}"#]);
    assert!(out.status.success(), "{out:?}");

    // Relay to a blocked worker whose turn ended: lifted to idle, woken with the line only.
    let w = "a-drelay";
    lab.worker(w, "ready");
    lab.hook(w, r#"{"hook_event_name":"UserPromptSubmit"}"#);
    lab.send(w, ORCH, "ask", "which db?");
    assert!(lab.hook(w, r#"{"hook_event_name":"Stop"}"#).is_empty());
    assert_eq!(lab.record(w)["status"], "blocked");
    assert_eq!(lab.record(w)["turn"], "ended");
    let word = lab.send(ORCH, w, "relay", "use postgres");
    assert!(word == "woken" || word == "scheduled", "relay: {word}");
    assert_eq!(lab.record(w)["status"], "idle", "relay must lift blocked");
    wait_until("relay wake line", 15, || lab.log(w).contains("📨 1 message"));
    assert_eq!(lab.log(w).trim(), "📨 1 message", "only the wake line is typed");
    // The submitted wake line fires UserPromptSubmit: the body is injected and acked.
    let ctx = lab.hook(w, r#"{"hook_event_name":"UserPromptSubmit"}"#);
    assert!(ctx.contains("use postgres"), "body not injected: {ctx}");
    assert_eq!(lab.unread(w), 0);

    // Answer inside the wake window (scenario B: the pane was just active): not lost, the
    // background waker types the line once the pane has been quiet.
    let w = "a-dfast";
    lab.record_worker(w);
    lab.hook(w, r#"{"hook_event_name":"UserPromptSubmit"}"#);
    lab.send(w, ORCH, "ask", "branch?");
    lab.hook(w, r#"{"hook_event_name":"Stop"}"#);
    lab.pane(w, "ready"); // fresh output: the pane is not quiet yet
    assert_eq!(lab.send(ORCH, w, "answer", "use main"), "scheduled");
    wait_until("waker wake line", 15, || lab.log(w).contains("📨 1 message"));
    assert!(!lab.log(w).contains("use main"), "body typed: {}", lab.log(w));

    // Answer mid-turn: no wake line while the turn runs; the Stop hook hands it over.
    let w = "a-dmid";
    lab.worker(w, "ready");
    lab.hook(w, r#"{"hook_event_name":"UserPromptSubmit"}"#);
    lab.send(w, ORCH, "ask", "schema?");
    assert_eq!(lab.send(ORCH, w, "answer", "use v2"), "turn-end");
    quiet_windows(2);
    assert_eq!(lab.log(w), "", "typed into a running turn");
    let stop = lab.hook(w, r#"{"hook_event_name":"Stop"}"#);
    let stop: Value = serde_json::from_str(&stop).unwrap_or_else(|e| panic!("{e}: {stop}"));
    assert_eq!(stop["decision"], "block");
    assert!(stop["reason"].as_str().unwrap().contains("use v2"), "{stop}");
    assert_eq!(lab.unread(w), 0);
    assert_eq!(lab.record(w)["turn"], "running", "a blocked stop continues the turn");
    assert!(lab.hook(w, r#"{"hook_event_name":"Stop","stop_hook_active":true}"#).is_empty());
    assert_eq!(lab.record(w)["turn"], "ended");
    quiet_windows(2);
    assert_eq!(lab.log(w), "", "a handed-over message must not wake the pane");

    // A worker that asked and finished anyway: its own done replaces blocked.
    let w = "a-ddone";
    lab.worker(w, "ready");
    lab.send(w, ORCH, "ask", "deploy?");
    lab.send(w, ORCH, "done", "PR opened");
    assert_eq!(lab.record(w)["status"], "done");

    // Permission dialog after an ask: blocked, then the parent's relay lifts it to idle,
    // but the turn runs: the dialog never receives a keystroke.
    let w = "a-dperm";
    lab.worker(w, "Do you want to proceed? ❯ 1. Yes  2. No");
    lab.hook(w, r#"{"hook_event_name":"UserPromptSubmit"}"#);
    lab.send(w, ORCH, "ask", "may I push?");
    lab.hook(w, r#"{"hook_event_name":"PermissionRequest"}"#);
    assert_eq!(lab.record(w)["status"], "blocked");
    assert_eq!(lab.send(ORCH, w, "relay", "yes go"), "turn-end");
    assert_eq!(lab.send(ORCH, w, "report", "fyi"), "turn-end");
    quiet_windows(3);
    assert_eq!(lab.log(w), "", "a permission dialog was answered");

    // A human draft left in an idle pane: never cleared; the wake line is appended to it and
    // Enter submits both (no `C-u`, Q21).
    let w = "a-ddraft";
    lab.worker(w, "ready");
    lab.hook(w, r#"{"hook_event_name":"UserPromptSubmit"}"#);
    lab.hook(w, r#"{"hook_event_name":"Stop"}"#);
    lab.tmux(&["send-keys", "-t", &format!("={w}:"), "-l", "my-draft"]);
    lab.send(ORCH, w, "relay", "rebase please");
    wait_until("wake line after the draft", 15, || lab.log(w).contains("📨 1 message"));
    assert_eq!(lab.log(w).trim(), "my-draft📨 1 message", "draft wiped or body typed");

    let sock = lab.tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(lab.tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    // Wakers still polling (a-dperm) see their pane gone and exit.
    lab.tmux(&["kill-server"]);
}
