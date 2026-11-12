//! Binary smoke test for `cdamux msg send|read` under a temp CDAMUX_HOME.

use std::process::Command;

#[test]
fn msg_send_read_ack_and_ask_status() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");

    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", &home)
            .env("CDAMUX_AGENT_ID", "a-cli02")
            .output()
            .expect("spawn cdamux")
    };

    assert!(run(&[
        "state",
        "set",
        "agents",
        "a-cli01",
        r#"{"id":"a-cli01","client":"claude","host":"c-orch","role":"orchestrator","status":"working"}"#,
    ])
    .status
    .success());
    assert!(run(&[
        "state",
        "set",
        "agents",
        "a-cli02",
        r#"{"id":"a-cli02","client":"claude","host":"c-w","role":"worker","parent":"a-cli01"}"#,
    ])
    .status
    .success());

    let send = run(&[
        "msg",
        "send",
        "--to",
        "a-cli01",
        "--kind",
        "ask",
        "which schema?",
    ]);
    assert!(
        send.status.success(),
        "send: {}",
        String::from_utf8_lossy(&send.stderr)
    );
    let mid = String::from_utf8_lossy(&send.stdout);
    assert!(mid.trim().starts_with("m-"), "{mid}");

    let blocked = run(&["state", "get", "agents", "a-cli02"]);
    assert!(blocked.status.success());
    assert!(
        String::from_utf8_lossy(&blocked.stdout).contains("blocked"),
        "{}",
        String::from_utf8_lossy(&blocked.stdout)
    );

    let read = run(&["msg", "read", "--agent", "a-cli01"]);
    assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    let out = String::from_utf8_lossy(&read.stdout);
    assert!(out.contains("which schema?"), "{out}");
    assert!(out.contains("\"kind\":\"ask\""), "{out}");

    let ack = run(&["msg", "read", "--agent", "a-cli01", "--ack"]);
    assert!(ack.status.success());
    let empty = run(&["msg", "read", "--agent", "a-cli01"]);
    assert!(empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stdout).trim().is_empty());

    let answer = Command::new(bin)
        .args([
            "msg",
            "send",
            "--from",
            "a-cli01",
            "--to",
            "a-cli02",
            "--kind",
            "answer",
            "use v2",
        ])
        .env("CDAMUX_HOME", &home)
        .output()
        .unwrap();
    assert!(
        answer.status.success(),
        "{}",
        String::from_utf8_lossy(&answer.stderr)
    );
    let cleared = run(&["state", "get", "agents", "a-cli02"]);
    assert!(String::from_utf8_lossy(&cleared.stdout).contains("idle"));

    assert!(home.join("inbox/a-cli01.jsonl").is_file());
    assert!(home.join("inbox/a-cli01.cursor").is_file());
}
