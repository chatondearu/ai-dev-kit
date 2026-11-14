//! Deprecated `cdamux sidebar` prints a redirect to `cdamux nav` (#337).

use std::process::Command;

#[test]
fn sidebar_prints_deprecation_and_exits_nonzero() {
    let out = Command::new(env!("CARGO_BIN_EXE_cdamux"))
        .args(["sidebar", "--client", "/dev/null"])
        .env("TMUX", "throwaway")
        .output()
        .expect("cdamux sidebar");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("deprecated"), "{err}");
    assert!(err.contains("cdamux nav"), "{err}");
    assert!(err.contains("M-n"), "{err}");
}
