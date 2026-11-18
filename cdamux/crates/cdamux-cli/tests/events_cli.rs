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
    // Orchestrator → worker: not an orchestrator inbox, no msg item.
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
    assert_eq!(types, ["status", "status", "msg", "reply"], "{all:?}");
    assert_eq!((all[1]["status"].as_str(), all[1]["prev"].as_str()), (Some("done"), Some("working")));
    assert_eq!(all[2]["text"], "PR ready");
    assert_eq!(all[3]["text"], "One PR ready.");
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
