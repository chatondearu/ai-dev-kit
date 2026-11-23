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
    let set_run = run(&[
        "state",
        "set",
        "runs",
        "r-fin",
        r#"{"id":"r-fin","status":"ok","session":""}"#,
    ]);
    assert!(
        set_run.status.success(),
        "{}",
        String::from_utf8_lossy(&set_run.stderr)
    );

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
    assert!(
        home.join("runs/r-fin.json").is_file(),
        "last-orch must not archive finished runs"
    );
}

/// Reboot path: no tmux server yet. The prune is skipped, the restore still runs, and
/// the destructive `state gc` fails closed (M2).
#[test]
fn restore_last_orch_without_tmux_server_skips_prune_and_gc_aborts() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", tmp.path().join("home"))
            .env("TMUX_TMPDIR", tmp.path())
            .env_remove("TMUX")
            .output()
            .expect("spawn cdamux")
    };
    let restore = run(&["restore", "--last-orch"]);
    let stderr = String::from_utf8_lossy(&restore.stderr);
    assert!(stderr.contains("skipping the unrestorable prune"), "{stderr}");
    assert!(stderr.contains("no recorded global orchestrator"), "{stderr}");
    let gc = run(&["state", "gc"]);
    assert_eq!(gc.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&gc.stderr).contains("aborting"));
}
