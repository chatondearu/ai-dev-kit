//! Binary smoke for `cdamux worker ls|peek` error paths and `spawn --here` without git worktree.

use std::process::Command;

#[test]
fn worker_requires_cdamux_agent_id() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let out = Command::new(bin)
        .args(["worker", "ls"])
        .env("CDAMUX_HOME", tmp.path())
        .env_remove("CDAMUX_AGENT_ID")
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("CDAMUX_AGENT_ID"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn worker_ls_empty_under_caller() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", &home)
            .env("CDAMUX_AGENT_ID", "a-orch9")
            .output()
            .expect("spawn cdamux")
    };
    assert!(run(&[
        "state",
        "set",
        "agents",
        "a-orch9",
        r#"{"id":"a-orch9","client":"claude","host":"c-orch","role":"orchestrator"}"#,
    ])
    .status
    .success());
    let ls = run(&["worker", "ls"]);
    assert!(ls.status.success(), "{}", String::from_utf8_lossy(&ls.stderr));
    assert!(String::from_utf8_lossy(&ls.stdout).trim().is_empty());
}
