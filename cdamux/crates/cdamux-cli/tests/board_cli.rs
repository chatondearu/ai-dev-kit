//! Binary smoke for `cdamux board` / `cdamux board refresh` on throwaway CDAMUX_HOME.

use std::fs;
use std::process::Command;

fn init_git(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    assert!(Command::new("git")
        .args(["-C", dir.to_str().unwrap(), "init", "-b", "main"])
        .status()
        .unwrap()
        .success());
}

#[test]
fn board_refresh_writes_index_and_cat_reads_it() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let roots = tmp.path().join("roots");
    let kit = roots.join("kit");
    let plain = roots.join("plain");
    let beamy = roots.join("beamy-v2");
    init_git(&kit);
    init_git(&plain);
    init_git(&beamy);
    fs::create_dir_all(kit.join("foam/board")).unwrap();
    fs::write(
        kit.join("foam/board/407-index.md"),
        "---\nstatus: ready\npriority: P1\nupdated: 2026-11-19\ntitle: \"index\"\n---\n",
    )
    .unwrap();
    fs::create_dir_all(beamy.join("foam/board")).unwrap();
    fs::write(
        beamy.join("foam/board/1-secret.md"),
        "---\nstatus: ready\npriority: P0\nupdated: 2026-11-19\n---\n",
    )
    .unwrap();

    let cfg = tmp.path().join("config.toml");
    fs::write(
        &cfg,
        format!(
            "schema_version = 1\n[projects]\ndiscover = \"git_only\"\nmax_depth = 4\n[[projects.roots]]\npath = \"{}\"\nlabel = \"t\"\n",
            roots.display()
        ),
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_cdamux");
    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("CDAMUX_HOME", &home)
            .env("CDAMUX_CONFIG", &cfg)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GH_PROMPT_DISABLED", "1")
            .output()
            .expect("spawn cdamux")
    };

    let missing = run(&["board"]);
    assert_eq!(missing.status.code(), Some(1), "cat before refresh");

    let refresh = run(&["board", "refresh"]);
    assert!(
        refresh.status.success(),
        "refresh: {}",
        String::from_utf8_lossy(&refresh.stderr)
    );
    let out = String::from_utf8_lossy(&refresh.stdout);
    assert!(out.contains("\"generated_at\""), "{out}");
    assert!(out.contains("\"ready\""), "{out}");
    assert!(out.contains("kit"), "{out}");
    assert!(out.contains("plain"), "{out}");
    assert!(!out.contains("beamy-v2"), "{out}");
    assert!(out.contains("missing foam"), "{out}");

    let index_path = home.join("board-index.json");
    assert!(index_path.is_file(), "atomic write landed");
    let file = fs::read_to_string(&index_path).unwrap();
    assert!(file.contains("\"projects\""));
    assert!(!file.contains("beamy-v2"));

    let cat = run(&["board"]);
    assert!(
        cat.status.success(),
        "cat: {}",
        String::from_utf8_lossy(&cat.stderr)
    );
    let cat_out = String::from_utf8_lossy(&cat.stdout);
    assert_eq!(cat_out, file);

    let help = run(&["board", "refresh", "--help"]);
    assert!(help.status.success());
    let help_txt = String::from_utf8_lossy(&help.stdout);
    assert!(help_txt.contains("--pull"), "{help_txt}");
}

#[test]
fn board_help_documents_cat_path() {
    let bin = env!("CARGO_BIN_EXE_cdamux");
    let out = Command::new(bin)
        .args(["board", "--help"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("board-index.json"), "{text}");
    assert!(text.contains("cdamux board refresh"), "{text}");
}
