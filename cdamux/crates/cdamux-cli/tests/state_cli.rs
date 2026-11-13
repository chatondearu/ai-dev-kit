//! Binary smoke test for `cdamux state set|get|append|gc` under a temp CDAMUX_HOME.

use std::fs;
use std::process::Command;
use std::time::{Duration, SystemTime};

#[test]
fn state_set_get_append_round_trip() {
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
        "a-cli01",
        r#"{"id":"a-cli01","client":"claude"}"#,
    ]);
    assert!(
        set.status.success(),
        "set: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let get = run(&["state", "get", "agents", "a-cli01"]);
    assert!(get.status.success(), "get: {}", String::from_utf8_lossy(&get.stderr));
    let out = String::from_utf8_lossy(&get.stdout);
    assert!(out.contains("claude"), "{out}");

    let append = run(&["state", "append", "agents", "a-cli01", r#"{"status":"idle"}"#]);
    assert!(
        append.status.success(),
        "append: {}",
        String::from_utf8_lossy(&append.stderr)
    );

    let get2 = run(&["state", "get", "agents", "a-cli01"]);
    assert!(get2.status.success());
    let out2 = String::from_utf8_lossy(&get2.stdout);
    assert!(out2.contains("idle"), "{out2}");
    assert!(out2.contains("claude"), "{out2}");
    assert!(home.join("agents/a-cli01.json").is_file());
}

#[test]
fn state_gc_archives_old_done_agent() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let cfg = tmp.path().join("config.toml");
    fs::write(
        &cfg,
        "schema_version = 1\n[state]\nretention_days = 14\n",
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_cdamux");

    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", &home)
            .env("CDAMUX_CONFIG", &cfg)
            .output()
            .expect("spawn cdamux")
    };

    let set = run(&[
        "state",
        "set",
        "agents",
        "a-oldgc",
        r#"{"id":"a-oldgc","status":"done"}"#,
    ]);
    assert!(
        set.status.success(),
        "set: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    let path = home.join("agents/a-oldgc.json");
    let mtime = SystemTime::now() - Duration::from_secs(20 * 24 * 60 * 60);
    fs::File::open(&path).unwrap().set_modified(mtime).unwrap();

    let gc = run(&["state", "gc"]);
    assert!(
        gc.status.success(),
        "gc: {}",
        String::from_utf8_lossy(&gc.stderr)
    );
    let out = String::from_utf8_lossy(&gc.stdout);
    assert!(out.contains("a-oldgc"), "{out}");
    assert!(!path.exists());
    assert!(home.join("archive/agents/a-oldgc.json").is_file());
}

#[test]
fn state_gc_rejects_zero_retention_days() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let gc = Command::new(bin)
        .args(["state", "gc", "--retention-days", "0"])
        .env("CDAMUX_HOME", &home)
        .output()
        .expect("spawn cdamux");
    assert!(
        !gc.status.success(),
        "expected clap reject for --retention-days 0"
    );
}

#[test]
fn state_gc_sweep_orphans_dry_run_then_finalize() {
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
        "a-orphan",
        r#"{"id":"a-orphan","host":"definitely-not-a-live-tmux-session","status":"working"}"#,
    ]);
    assert!(
        set.status.success(),
        "set: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let dry = run(&["state", "gc", "--sweep-orphans", "--dry-run"]);
    assert!(
        dry.status.success(),
        "dry-run: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let dry_out = String::from_utf8_lossy(&dry.stdout);
    assert!(dry_out.contains("would finalize"), "{dry_out}");
    assert!(dry_out.contains("a-orphan"), "{dry_out}");
    let still = fs::read_to_string(home.join("agents/a-orphan.json")).unwrap();
    assert!(still.contains("working"), "{still}");

    let sweep = run(&["state", "gc", "--sweep-orphans"]);
    assert!(
        sweep.status.success(),
        "sweep: {}",
        String::from_utf8_lossy(&sweep.stderr)
    );
    let after = fs::read_to_string(home.join("agents/a-orphan.json")).unwrap();
    assert!(after.contains(r#""status": "error""#) || after.contains(r#""status":"error"#), "{after}");
}

#[test]
fn state_gc_aborts_when_tmux_unavailable() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let bin = env!("CARGO_BIN_EXE_cdamux");

    let set = Command::new(bin)
        .args([
            "state",
            "set",
            "agents",
            "a-notmux",
            r#"{"id":"a-notmux","status":"done"}"#,
        ])
        .env("CDAMUX_HOME", &home)
        .output()
        .expect("spawn cdamux");
    assert!(set.status.success());
    let path = home.join("agents/a-notmux.json");
    let mtime = SystemTime::now() - Duration::from_secs(20 * 24 * 60 * 60);
    fs::File::open(&path).unwrap().set_modified(mtime).unwrap();

    let gc = Command::new(bin)
        .args(["state", "gc", "--retention-days", "1"])
        .env("CDAMUX_HOME", &home)
        .env("PATH", "/nonexistent")
        .output()
        .expect("spawn cdamux");
    assert!(
        !gc.status.success(),
        "gc without tmux should abort: {}",
        String::from_utf8_lossy(&gc.stderr)
    );
    let err = String::from_utf8_lossy(&gc.stderr);
    assert!(
        err.contains("cannot probe tmux") || err.contains("tmux"),
        "{err}"
    );
    assert!(
        path.exists(),
        "agent must remain when gc aborts on tmux failure"
    );
}
