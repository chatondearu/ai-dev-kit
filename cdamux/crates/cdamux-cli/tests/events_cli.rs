//! Binary smoke test for `cdamux events` under a temp CDAMUX_HOME: replay, cursor,
//! follow, the turn-end reply hook, and inbox ack cursors left untouched.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn events_replay_follow_and_reply() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let cmd = |args: &[&str]| {
        let mut c = Command::new(bin);
        c.args(args).env("CDAMUX_HOME", &home).env("CDAMUX_AGENT_ID", "a-ew");
        c
    };
    let run = |args: &[&str]| {
        let out = cmd(args).output().expect("spawn cdamux");
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let items = |since: &str| -> Vec<serde_json::Value> {
        run(&["events", "--since", since, "--json"])
            .lines()
            .map(|l| serde_json::from_str(l).expect("JSON line"))
            .collect()
    };

    run(&["state", "set", "agents", "a-eo", r#"{"id":"a-eo","role":"orchestrator"}"#]);
    run(&["state", "set", "agents", "a-ew", r#"{"id":"a-ew","role":"worker","parent":"a-eo"}"#]);
    run(&["state", "append", "agents", "a-ew", r#"{"status":"working"}"#]);
    run(&["msg", "send", "--to", "a-eo", "--kind", "done", "PR ready"]);
    // Orchestrator → worker: not an orchestrator inbox, no msg item. The relay does lift
    // the worker's sticky `done` to `idle` (ADR-018 amendment, #444): one status item.
    run(&["msg", "send", "--from", "a-eo", "--to", "a-ew", "--kind", "relay", "thanks"]);

    let transcript = tmp.path().join("t.jsonl");
    std::fs::write(
        &transcript,
        concat!(
            r#"{"type":"user","uuid":"u1","message":{"role":"user","content":"status?"}}"#,
            "\n",
            r#"{"type":"assistant","uuid":"x1","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{}}]}}"#,
            "\n",
            r#"{"type":"user","uuid":"u2","message":{"role":"user","content":[{"type":"tool_result","content":"noise"}]}}"#,
            "\n",
            r#"{"type":"assistant","uuid":"x2","message":{"role":"assistant","content":[{"type":"text","text":"One PR ready."}]}}"#,
        ),
    )
    .unwrap();
    let payload = serde_json::json!({"hook_event_name":"Stop","transcript_path":transcript});
    let mut hook = cmd(&["events", "reply", "a-eo"]).stdin(Stdio::piped()).spawn().unwrap();
    hook.stdin.take().unwrap().write_all(payload.to_string().as_bytes()).unwrap();
    assert!(hook.wait().unwrap().success());

    let unread_before = run(&["msg", "read", "--agent", "a-eo"]);
    let all = items("0");
    assert!(all.iter().all(|v| v["v"] == 1 && v["cursor"].is_string()), "{all:?}");
    let types: Vec<&str> = all.iter().map(|v| v["type"].as_str().unwrap()).collect();
    assert_eq!(types, ["status", "status", "msg", "status", "reply"], "{all:?}");
    assert_eq!((all[1]["status"].as_str(), all[1]["prev"].as_str()), (Some("done"), Some("working")));
    assert_eq!(all[2]["text"], "PR ready");
    assert_eq!((all[3]["status"].as_str(), all[3]["prev"].as_str()), (Some("idle"), Some("done")));
    assert_eq!(all[4]["text"], "One PR ready.");
    // Reading the feed never moves an inbox cursor.
    assert_eq!(run(&["msg", "read", "--agent", "a-eo"]), unread_before);
    assert!(unread_before.contains("PR ready"));

    let last = all.last().unwrap()["cursor"].as_str().unwrap().to_string();
    assert!(items(&last).is_empty());
    let human = run(&["events", "--orchestrators", "--since", "0"]);
    assert!(human.lines().count() == 2 && human.contains("a-ew→a-eo [done] PR ready"), "{human}");

    let mut follow = cmd(&["events", "--follow", "--json", "--since", &last])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = follow.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    run(&["msg", "send", "--to", "a-eo", "--kind", "ask", "which schema?"]);
    let mut seen = Vec::new();
    while let Ok(line) = rx.recv_timeout(Duration::from_secs(3)) {
        seen.push(line);
        if seen.iter().any(|l| l.contains("which schema?")) {
            break;
        }
    }
    let _ = follow.kill();
    let _ = follow.wait();
    assert!(seen.iter().any(|l| l.contains("which schema?")), "follow saw: {seen:?}");

    let bad = cmd(&["events", "--since", "999"]).output().unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("ahead"));
}

/// Pinned for clients (neomind): a refused cursor exits 3 with a stable stderr prefix,
/// distinct from an I/O error (1) and a malformed cursor (2).
#[test]
fn refused_cursor_has_its_own_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let run = |args: &[&str]| Command::new(bin).args(args).env("CDAMUX_HOME", &home).output().unwrap();
    let refused = |out: &std::process::Output| {
        assert_eq!(out.status.code(), Some(3), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(out.stdout.is_empty());
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(err.starts_with("events: cursor refused: "), "{err}");
        err
    };

    // Ahead of an empty journal (reset home), with and without --follow.
    assert!(refused(&run(&["events", "--since", "5"])).contains("ahead"));
    assert!(refused(&run(&["events", "--follow", "--since", "5", "--json"])).contains("ahead"));

    // Expired: older than the oldest retained item (rotated-away segment).
    std::fs::create_dir_all(&home).unwrap();
    let line = |c: u64| format!("{{\"v\":1,\"cursor\":\"{c}\",\"type\":\"msg\",\"ts\":1}}\n");
    std::fs::write(home.join("events.jsonl"), line(10) + &line(11)).unwrap();
    assert!(refused(&run(&["events", "--since", "3"])).contains("expired"));
    assert!(run(&["events", "--since", "9"]).status.success(), "just before oldest is fine");

    assert_eq!(run(&["events", "--since", "x"]).status.code(), Some(2));
}

/// `--follow` notices a journal reset in place (same inode) that regrew past its read
/// offset before the next poll, and refuses its cursor instead of streaming garbage.
#[test]
fn follow_refuses_an_in_place_reset() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let journal = home.join("events.jsonl");
    let line = |c: u64, text: &str| {
        format!("{{\"v\":1,\"cursor\":\"{c}\",\"type\":\"msg\",\"ts\":1,\"text\":\"{text}\"}}\n")
    };
    std::fs::write(&journal, line(1, "a") + &line(2, "b") + &line(3, "c")).unwrap();
    let mut follow = Command::new(env!("CARGO_BIN_EXE_cdamux"))
        .args(["events", "--follow", "--json", "--since", "0"])
        .env("CDAMUX_HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(follow.stdout.take().unwrap());
    for _ in 0..3 {
        let mut l = String::new();
        out.read_line(&mut l).unwrap();
        assert!(l.contains("\"cursor\""), "{l}");
    }
    std::fs::write(&journal, line(1, &"x".repeat(400))).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(s) = follow.try_wait().unwrap() {
            break Some(s);
        }
        if std::time::Instant::now() > deadline {
            let _ = follow.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    std::io::Read::read_to_string(&mut follow.stderr.take().unwrap(), &mut err).unwrap();
    assert_eq!(status.and_then(|s| s.code()), Some(3), "stderr: {err}");
    assert!(err.starts_with("events: cursor refused: ") && err.contains("ahead"), "{err}");
}
