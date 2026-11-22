//! Binary smoke for `cdamux orch new`.

use std::process::Command;

#[test]
fn orch_new_help_lists_skill() {
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let out = Command::new(bin)
        .args(["orch", "new", "--help"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("cda-orchestrator") || text.contains("orchestrator"),
        "{text}"
    );
    assert!(text.contains("--child"), "{text}");
    assert!(text.contains("--global"), "{text}");
}

#[test]
fn restore_last_orch_prunes_unrestorable_error_stub() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", &home)
            .output()
            .expect("spawn cdamux")
    };

    let set = run(&[
        "state",
        "set",
        "agents",
        "a-stale",
        r#"{"id":"a-stale","role":"orchestrator","scope":"global","status":"error","host":""}"#,
    ]);
    assert!(set.status.success(), "{}", String::from_utf8_lossy(&set.stderr));

    let restore = run(&["restore", "--last-orch"]);
    let stdout = String::from_utf8_lossy(&restore.stdout);
    let stderr = String::from_utf8_lossy(&restore.stderr);
    assert!(
        !restore.status.success(),
        "no registry orch should fail: {stdout}{stderr}"
    );
    assert!(
        stderr.contains("no recorded global orchestrator") || stdout.contains("no recorded global orchestrator"),
        "stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("pruned") && stdout.contains("1 unrestorable"),
        "{stdout}"
    );
    assert!(!home.join("agents/a-stale.json").exists());
    assert!(home.join("archive/agents/a-stale.json").is_file());
}
