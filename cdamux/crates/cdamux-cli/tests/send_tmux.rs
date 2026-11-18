//! `cdamux send --key/--pane/--json` on a throwaway tmux server (cdamux/AGENTS.md
//! § Testing against tmux): C-c interrupts a running command, refused keys and unknown
//! panes exit 2, a missing session reports `session_not_found`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-send-test";

fn which(cmd: &str) -> Option<String> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|d| Path::new(d).join(cmd))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
}

/// Temp dir with a `bin/tmux` wrapper script; `body` is the script after the shebang.
fn sandbox(body: &str) -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(&wrapper, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    (tmp, path)
}

fn run(path: &str, tmpdir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cdamux"))
        .args(args)
        .env("PATH", path)
        .env("TMUX_TMPDIR", tmpdir)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("cdamux")
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {out:?}"))
}

#[test]
fn refused_keys_exit_2_without_calling_tmux() {
    let (tmp, path) = sandbox("touch \"$(dirname \"$0\")/called\"; exit 1");
    for bad in ["Foo", "C-c; kill-server", "c-c"] {
        let out = run(&path, tmp.path(), &["send", "t1", "--key", bad, "--json"]);
        assert_eq!(out.status.code(), Some(2), "{bad}: {out:?}");
    }
    assert!(!tmp.path().join("bin/called").exists(), "tmux was called");
}

#[test]
fn key_c_c_interrupts_and_json_reports_outcome() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    let (tmp, path) = sandbox(&format!("exec {real_tmux} -L {SOCKET} -f /dev/null \"$@\""));
    let tmux = |args: &[&str]| -> String {
        let out = Command::new("tmux")
            .args(args)
            .env("PATH", &path)
            .env("TMUX_TMPDIR", tmp.path())
            .env_remove("TMUX")
            .output()
            .expect("tmux");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    tmux(&["new-session", "-d", "-s", "t1", "-x", "120", "-y", "30", "--", "/bin/sh"]);
    let socket = PathBuf::from(tmux(&["display", "-p", "#{socket_path}"]).trim());
    let root = tmp.path().canonicalize().unwrap();
    assert!(socket.canonicalize().unwrap().starts_with(&root), "not a throwaway server: {}", socket.display());

    let result = std::panic::catch_unwind(|| {
        let out = run(&path, tmp.path(), &["send", "t1", "sleep 30", "--enter"]);
        assert!(out.status.success(), "{out:?}");
        std::thread::sleep(Duration::from_millis(500));

        let out = run(&path, tmp.path(), &["send", "t1", "--key", "C-c", "--json"]);
        assert!(out.status.success(), "{out:?}");
        let v = json(&out);
        assert_eq!(v["v"], 1);
        assert_eq!(v["session"], "t1");
        assert_eq!(v["outcome"], "ok");
        assert_eq!(v["keys"], serde_json::json!(["C-c"]));
        assert_eq!(v["sent_bytes"], 0);
        let pane = v["pane"].as_str().unwrap().to_string();
        assert!(pane.starts_with('%'), "{v}");

        // The shell is back at its prompt only if `sleep 30` was interrupted.
        let out = run(&path, tmp.path(), &["send", "t1", "--pane", &pane, "echo DONE-$((40+2))", "--enter"]);
        assert!(out.status.success(), "{out:?}");
        let start = Instant::now();
        while !tmux(&["capture-pane", "-p", "-t", &pane]).contains("DONE-42") {
            assert!(start.elapsed() < Duration::from_secs(10), "C-c did not interrupt:\n{}", tmux(&["capture-pane", "-p", "-t", &pane]));
            std::thread::sleep(Duration::from_millis(100));
        }

        let out = run(&path, tmp.path(), &["send", "t1", "--pane", "%999", "--key", "Enter"]);
        assert_eq!(out.status.code(), Some(2), "{out:?}");

        let out = run(&path, tmp.path(), &["send", "nope", "--key", "C-c", "--json"]);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert_eq!(json(&out)["outcome"], "session_not_found");
    });
    tmux(&["kill-server"]);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
