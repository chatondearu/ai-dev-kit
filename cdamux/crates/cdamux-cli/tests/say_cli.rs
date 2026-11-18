//! Binary smoke test for `cdamux say` and `cdamux msg answer` under a temp CDAMUX_HOME
//! (no tmux: the orchestrator is busy, so `say` queues).

use std::process::{Command, Output};

#[test]
fn say_and_msg_answer_exit_codes_json_and_events() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let run = |args: &[&str]| -> Output {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", &home)
            .env("CDAMUX_AGENT_ID", "a-sw")
            .output()
            .expect("spawn cdamux")
    };
    let ok = |args: &[&str]| {
        let out = run(args);
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let json = |s: &str| serde_json::from_str::<serde_json::Value>(s).expect("JSON");

    ok(&["state", "set", "agents", "a-so", r#"{"id":"a-so","role":"orchestrator","status":"working"}"#]);
    ok(&["state", "set", "agents", "a-sw", r#"{"id":"a-sw","role":"worker","parent":"a-so","status":"working"}"#]);

    // Queued: `<id> queued`, and the --json shape.
    let plain = ok(&["say", "a-so", "--from", "alice", "status", "please?"]);
    assert!(plain.trim().starts_with("m-") && plain.trim().ends_with(" queued"), "{plain}");
    let v = json(&ok(&["say", "a-so", "--from", "alice", "--json", "again"]));
    assert_eq!((v["v"].as_u64(), v["target"].as_str(), v["outcome"].as_str()), (Some(1), Some("a-so"), Some("queued")));
    assert!(v["id"].as_str().is_some_and(|id| id.starts_with("m-")));

    // Worker and unknown targets: exit 2, the worker one points to `msg send`.
    let worker = run(&["say", "a-sw", "--from", "alice", "hi"]);
    assert_eq!(worker.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&worker.stderr).contains("msg send"));
    assert_eq!(run(&["say", "a-nope", "--from", "alice", "hi"]).status.code(), Some(2));

    // One `human` event per accepted say.
    let events = ok(&["events", "--since", "0", "--json"]);
    let human: Vec<_> = events.lines().map(json).filter(|e| e["type"] == "human").collect();
    assert_eq!(human.len(), 2);
    assert_eq!((human[0]["agent"].as_str(), human[0]["from"].as_str()), (Some("a-so"), Some("alice")));
    assert_eq!(human[0]["text"], "status please?");

    // msg answer: unblocks the worker, relay copy in the orchestrator inbox, repeat → exit 2.
    let ask = ok(&["msg", "send", "--to", "a-so", "--kind", "ask", "which schema?"]);
    let ask_id = ask.split_whitespace().next().unwrap().to_string();
    assert_eq!(run(&["msg", "answer", "m-nope0", "--from", "alice", "v2"]).status.code(), Some(2));
    let v = json(&ok(&["msg", "answer", &ask_id, "--from", "alice", "--json", "use", "v2"]));
    assert_eq!((v["ask"].as_str(), v["worker"].as_str(), v["outcome"].as_str()), (Some(ask_id.as_str()), Some("a-sw"), Some("answered")));
    let again = run(&["msg", "answer", &ask_id, "--from", "alice", "use v3"]);
    assert_eq!(again.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&again.stderr).contains("already answered"));
    let worker_inbox = ok(&["msg", "read", "--agent", "a-sw"]);
    assert!(worker_inbox.contains(r#""kind":"answer""#) && worker_inbox.contains("[alice] use v2"), "{worker_inbox}");
    let orch_inbox = ok(&["msg", "read", "--agent", "a-so"]);
    assert!(orch_inbox.contains(r#""kind":"relay""#), "{orch_inbox}");
    let state = ok(&["state", "get", "agents", "a-sw"]);
    assert!(!state.contains("blocked"), "{state}");
}
