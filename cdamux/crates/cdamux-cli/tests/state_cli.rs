//! Binary smoke test for `cdamux state set|get|append` under a temp CDAMUX_HOME.

use std::process::Command;

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
