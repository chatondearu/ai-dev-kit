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
}
