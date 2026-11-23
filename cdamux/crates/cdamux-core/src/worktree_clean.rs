//! Free build caches in finished worktrees (`cdamux worker clean`) and the spawn disk guard.
//!
//! Only git-ignored directories named in `[worktrees].disposable` are removed; the
//! worktree, its branch and tracked or untracked work stay (ADR-017).

use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Default `[worktrees].disposable`: Rust and Node build output.
pub const DEFAULT_DISPOSABLE: &[&str] = &["target", "node_modules", ".nuxt", ".output"];
/// Default `[worktrees].min_free_gib` checked at `worker spawn` (`0` = off).
pub const DEFAULT_MIN_FREE_GIB: u64 = 40;

const GIB: u64 = 1024 * 1024 * 1024;

/// A disposable entry is a relative path inside the worktree: no absolute path, no `..`.
pub fn validate_disposable(entry: &str) -> Result<(), String> {
    let p = Path::new(entry);
    let ok = !entry.trim().is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "[worktrees].disposable entry '{entry}' must be a relative path without '..'"
        ))
    }
}

/// GitHub PR state for a worktree branch (`gh pr list --head`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
    /// No PR for the branch (or no branch).
    None,
    /// `gh` missing or failed: treated as "maybe open".
    Unknown,
}

/// tmux sessions rooted in the worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeSession {
    /// At least one live session whose agent is not `done`.
    Live,
    /// Live sessions only, all with agent status `done`.
    Done,
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Clean(&'static str),
    Keep(&'static str),
}

/// Whether a worktree is finished for good. Open PRs and live unfinished sessions always keep.
pub fn clean_verdict(session: WorktreeSession, pr: PrState, branch_merged: bool) -> Verdict {
    match (session, pr) {
        (WorktreeSession::Live, _) => Verdict::Keep("session live"),
        (_, PrState::Open) => Verdict::Keep("PR open"),
        (_, PrState::Unknown) => Verdict::Keep("PR state unknown (gh)"),
        (_, PrState::Merged) => Verdict::Clean("PR merged"),
        (_, PrState::Closed) => Verdict::Clean("PR closed"),
        (WorktreeSession::Gone, PrState::None) if branch_merged => {
            Verdict::Clean("branch merged, session gone")
        }
        (_, PrState::None) => Verdict::Keep("no PR, branch not merged or session done"),
    }
}

/// `gh pr list --json state` output → one state (any open PR wins).
pub fn parse_gh_pr_states(json: &str) -> PrState {
    let Ok(Value::Array(prs)) = serde_json::from_str::<Value>(json) else {
        return PrState::Unknown;
    };
    let has = |s: &str| {
        prs.iter()
            .any(|p| p.get("state").and_then(|v| v.as_str()) == Some(s))
    };
    if has("OPEN") {
        PrState::Open
    } else if has("MERGED") {
        PrState::Merged
    } else if has("CLOSED") {
        PrState::Closed
    } else if prs.is_empty() {
        PrState::None
    } else {
        PrState::Unknown
    }
}

/// Available KiB from `df -Pk` (the column before `Capacity`, which ends in `%`).
pub fn parse_df_avail_kib(out: &str) -> Option<u64> {
    let line = out.lines().nth(1)?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    let cap = cols.iter().position(|c| c.ends_with('%'))?;
    cols.get(cap.checked_sub(1)?)?.parse().ok()
}

/// Free bytes on the filesystem holding `path` (its nearest existing ancestor).
pub fn free_bytes(path: &Path) -> Option<u64> {
    let dir = path.ancestors().find(|p| p.exists())?;
    let out = Command::new("df").arg("-Pk").arg(dir).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_df_avail_kib(&String::from_utf8_lossy(&out.stdout)).map(|k| k * 1024)
}

/// Refuse when `avail` is below `min_gib` GiB (`0` = off).
pub fn disk_guard(avail: u64, min_gib: u64, path: &Path) -> Result<(), String> {
    if min_gib == 0 || avail >= min_gib.saturating_mul(GIB) {
        return Ok(());
    }
    Err(format!(
        "low disk: {} free on {} (< {min_gib} GiB, [worktrees].min_free_gib); \
         free build caches with `cdamux worker clean --dry-run` then `cdamux worker clean`",
        format_bytes(avail),
        path.display()
    ))
}

/// `disk_guard` on the live filesystem; fail-open when `df` cannot be read.
pub fn check_free_space(path: &Path, min_gib: u64) -> Result<(), String> {
    if min_gib == 0 {
        return Ok(());
    }
    match free_bytes(path) {
        Some(avail) => disk_guard(avail, min_gib, path),
        None => Ok(()),
    }
}

pub fn format_bytes(b: u64) -> String {
    if b >= GIB {
        format!("{:.1} GiB", b as f64 / GIB as f64)
    } else {
        format!("{:.0} MiB", b as f64 / (1024.0 * 1024.0))
    }
}

/// Entry without `/` matches a directory name at any depth; otherwise the exact relative path.
fn matches_disposable(rel: &str, patterns: &[String]) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    patterns.iter().any(|p| {
        let p = p.trim_end_matches('/');
        if p.contains('/') {
            rel == p
        } else {
            name == p
        }
    })
}

fn git_out(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Git-ignored directories of `wt` matching `patterns`, each a real directory inside `wt`.
pub fn disposable_dirs(wt: &Path, patterns: &[String]) -> Vec<PathBuf> {
    if patterns.is_empty() {
        return Vec::new();
    }
    let Ok(root) = wt.canonicalize() else {
        return Vec::new();
    };
    let Some(out) = git_out(
        wt,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ],
    ) else {
        return Vec::new();
    };
    out.split('\0')
        .filter_map(|e| e.strip_suffix('/'))
        .filter(|rel| validate_disposable(rel).is_ok() && matches_disposable(rel, patterns))
        .map(|rel| wt.join(rel))
        .filter(|p| is_real_dir_inside(p, &root))
        .collect()
}

/// Not a symlink, and its resolved path stays strictly under `root`.
fn is_real_dir_inside(p: &Path, root: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(p) else {
        return false;
    };
    if !meta.is_dir() {
        return false;
    }
    p.canonicalize()
        .is_ok_and(|c| c != root && c.starts_with(root))
}

/// Disk usage of a tree (allocated blocks; symlinks are not followed).
pub fn dir_size(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(meta) = fs::symlink_metadata(&p) else {
            continue;
        };
        total += meta.blocks() * 512;
        if meta.is_dir() {
            if let Ok(rd) = fs::read_dir(&p) {
                stack.extend(rd.flatten().map(|e| e.path()));
            }
        }
    }
    total
}

/// Linked git worktrees under `root`: `<root>/<name>` (orch.sh) or `<root>/<run>/<name>`
/// (cdamux). Main checkouts (`.git` directory) are never candidates.
pub fn find_worktrees(root: &Path) -> Vec<PathBuf> {
    fn children(p: &Path) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(p)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }
    let is_wt = |p: &Path| p.join(".git").is_file();
    let mut out = Vec::new();
    for c in children(root) {
        if is_wt(&c) {
            out.push(c);
        } else {
            out.extend(children(&c).into_iter().filter(|g| is_wt(g)));
        }
    }
    out
}

fn branch_name(wt: &Path) -> Option<String> {
    git_out(wt, &["symbolic-ref", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `HEAD` is already contained in the default branch (local refs, no fetch).
fn branch_merged(wt: &Path) -> bool {
    let base = [
        "origin/HEAD",
        "origin/main",
        "origin/master",
        "main",
        "master",
    ]
    .into_iter()
    .find(|b| git_out(wt, &["rev-parse", "--verify", "-q", b]).is_some());
    base.is_some_and(|b| {
        Command::new("git")
            .arg("-C")
            .arg(wt)
            .args(["merge-base", "--is-ancestor", "HEAD", b])
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn pr_state(wt: &Path) -> PrState {
    let Some(branch) = branch_name(wt) else {
        return PrState::None;
    };
    let out = Command::new("gh")
        .current_dir(wt)
        .args([
            "pr", "list", "--head", &branch, "--state", "all", "--json", "state", "--limit", "20",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_gh_pr_states(&String::from_utf8_lossy(&o.stdout)),
        _ => PrState::Unknown,
    }
}

/// Live tmux session rooted in `wt` → `Live` unless every such session's agent is `done`.
pub fn session_state(
    wt: &Path,
    sessions: &[(String, PathBuf)],
    agent_status: &dyn Fn(&str) -> Option<String>,
) -> WorktreeSession {
    let root = wt.canonicalize().ok();
    let mut inside = sessions.iter().filter(|(_, p)| {
        p.starts_with(wt)
            || root
                .as_ref()
                .is_some_and(|r| p.canonicalize().is_ok_and(|c| c.starts_with(r)))
    });
    let Some(first) = inside.next() else {
        return WorktreeSession::Gone;
    };
    let done = |host: &str| agent_status(host).as_deref() == Some("done");
    if done(&first.0) && inside.all(|(h, _)| done(h)) {
        WorktreeSession::Done
    } else {
        WorktreeSession::Live
    }
}

/// One worktree's outcome in a `worker clean` pass.
#[derive(Debug, Clone)]
pub struct CleanItem {
    pub worktree: PathBuf,
    pub verdict: Verdict,
    /// Disposable dirs with their size (removed unless dry-run or kept).
    pub dirs: Vec<(PathBuf, u64)>,
    /// Removal failures (`path: error`).
    pub errors: Vec<String>,
}

impl CleanItem {
    pub fn bytes(&self) -> u64 {
        self.dirs.iter().map(|(_, s)| s).sum()
    }
}

/// Scan `roots`, decide per worktree, remove disposable dirs of finished ones (unless `dry_run`).
/// Worktrees with nothing to free are skipped without calling `gh`.
pub fn clean_worktrees(
    roots: &[PathBuf],
    patterns: &[String],
    dry_run: bool,
    sessions: &[(String, PathBuf)],
    agent_status: &dyn Fn(&str) -> Option<String>,
) -> Vec<CleanItem> {
    let mut items = Vec::new();
    for wt in roots.iter().flat_map(|r| find_worktrees(r)) {
        let found = disposable_dirs(&wt, patterns);
        if found.is_empty() {
            continue;
        }
        let session = session_state(&wt, sessions, agent_status);
        let verdict = if session == WorktreeSession::Live {
            clean_verdict(session, PrState::Unknown, false)
        } else {
            clean_verdict(session, pr_state(&wt), branch_merged(&wt))
        };
        let dirs: Vec<(PathBuf, u64)> = found
            .into_iter()
            .map(|d| {
                let s = dir_size(&d);
                (d, s)
            })
            .collect();
        let mut errors = Vec::new();
        if !dry_run && matches!(verdict, Verdict::Clean(_)) {
            for (d, _) in &dirs {
                if let Err(e) = fs::remove_dir_all(d) {
                    errors.push(format!("{}: {e}", d.display()));
                }
            }
        }
        items.push(CleanItem {
            worktree: wt,
            verdict,
            dirs,
            errors,
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn validate_disposable_refuses_absolute_and_parent() {
        for ok in ["target", "node_modules", "cdamux/target", ".nuxt"] {
            assert!(validate_disposable(ok).is_ok(), "{ok}");
        }
        for bad in ["", " ", "/tmp", "../x", "a/../b", "./target", "."] {
            assert!(validate_disposable(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn verdict_keeps_open_pr_and_live_session() {
        use PrState::*;
        use WorktreeSession::*;
        assert_eq!(
            clean_verdict(Live, Merged, true),
            Verdict::Keep("session live")
        );
        assert_eq!(clean_verdict(Gone, Open, true), Verdict::Keep("PR open"));
        assert_eq!(clean_verdict(Done, Open, true), Verdict::Keep("PR open"));
        assert!(matches!(
            clean_verdict(Gone, Unknown, true),
            Verdict::Keep(_)
        ));
        assert_eq!(
            clean_verdict(Gone, Merged, false),
            Verdict::Clean("PR merged")
        );
        assert_eq!(
            clean_verdict(Done, Merged, false),
            Verdict::Clean("PR merged")
        );
        assert_eq!(
            clean_verdict(Gone, Closed, false),
            Verdict::Clean("PR closed")
        );
        assert!(matches!(clean_verdict(Gone, None, true), Verdict::Clean(_)));
        assert!(matches!(clean_verdict(Gone, None, false), Verdict::Keep(_)));
        assert!(matches!(clean_verdict(Done, None, true), Verdict::Keep(_)));
    }

    #[test]
    fn gh_states_open_wins() {
        assert_eq!(
            parse_gh_pr_states(r#"[{"state":"MERGED"},{"state":"OPEN"}]"#),
            PrState::Open
        );
        assert_eq!(
            parse_gh_pr_states(r#"[{"state":"CLOSED"},{"state":"MERGED"}]"#),
            PrState::Merged
        );
        assert_eq!(
            parse_gh_pr_states(r#"[{"state":"CLOSED"}]"#),
            PrState::Closed
        );
        assert_eq!(parse_gh_pr_states("[]"), PrState::None);
        assert_eq!(parse_gh_pr_states("oops"), PrState::Unknown);
    }

    #[test]
    fn df_avail_parses_macos_and_linux() {
        let mac = "Filesystem   1024-blocks      Used Available Capacity  Mounted on\n\
                   /dev/disk3s8   971298980  87255088 106881148    45%    /cda\n";
        assert_eq!(parse_df_avail_kib(mac), Some(106881148));
        let spaced = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                      map auto home 0 0 0 100% /System/Volumes/Data/home\n";
        assert_eq!(parse_df_avail_kib(spaced), Some(0));
        assert_eq!(parse_df_avail_kib("garbage"), None);
    }

    #[test]
    fn disk_guard_threshold() {
        let p = Path::new("/wt");
        assert!(disk_guard(0, 0, p).is_ok());
        assert!(disk_guard(40 * GIB, 40, p).is_ok());
        let err = disk_guard(39 * GIB, 40, p).unwrap_err();
        assert!(err.contains("cdamux worker clean"), "{err}");
        assert!(err.contains("39.0 GiB"), "{err}");
    }

    fn git(dir: &Path, args: &[&str]) {
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            st.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&st.stderr)
        );
    }

    #[test]
    fn disposable_dirs_only_ignored_and_inside() {
        let tmp = tempfile::tempdir().unwrap();
        // A main checkout under the root is never scanned; only its linked worktree is.
        let main = tmp.path().join("main");
        fs::create_dir_all(main.join("target")).unwrap();
        git(&main, &["init", "-q"]);
        fs::write(main.join(".gitignore"), "target/\n").unwrap();
        git(&main, &["add", ".gitignore"]);
        git(
            &main,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-qm",
                "init",
            ],
        );
        let wt = tmp.path().join("run").join("wt");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "w", wt.to_str().unwrap()],
        );
        fs::write(
            wt.join(".gitignore"),
            "target/\nnode_modules/\nsub/target/\nlink\n",
        )
        .unwrap();
        fs::create_dir_all(wt.join("target/debug")).unwrap();
        fs::write(wt.join("target/debug/big"), vec![0u8; 8192]).unwrap();
        fs::create_dir_all(wt.join("sub/target")).unwrap();
        fs::write(wt.join("sub/target/x"), "x").unwrap();
        // Tracked `node_modules` must survive: not git-ignored output.
        fs::create_dir_all(wt.join("vendor/node_modules")).unwrap();
        fs::write(wt.join("vendor/node_modules/keep"), "k").unwrap();
        git(
            &wt,
            &["add", "-f", "vendor/node_modules/keep", ".gitignore"],
        );
        // Symlink to a dir outside the worktree is never followed.
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, wt.join("link")).unwrap();

        let pats = vec!["target".to_string(), "node_modules".into(), "link".into()];
        let mut got = disposable_dirs(&wt, &pats);
        got.sort();
        assert_eq!(got, vec![wt.join("sub/target"), wt.join("target")]);
        assert!(dir_size(&wt.join("target")) >= 8192);
        assert_eq!(find_worktrees(tmp.path()), vec![wt.clone()]);

        // Dry-run removes nothing; no sessions and no PR in a remote-less repo → kept.
        let items = clean_worktrees(&[tmp.path().to_path_buf()], &pats, true, &[], &|_| None);
        assert_eq!(items.len(), 1);
        assert!(wt.join("target").exists() && outside.exists());

        // A live unfinished session keeps the worktree, even when not a dry run.
        let sessions = vec![("w".to_string(), wt.clone())];
        let items = clean_worktrees(
            &[tmp.path().to_path_buf()],
            &pats,
            false,
            &sessions,
            &|_| Some("working".into()),
        );
        assert_eq!(items[0].verdict, Verdict::Keep("session live"));
        assert!(wt.join("target").exists());
    }

    #[test]
    fn session_state_done_and_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let wt = tmp.path().to_path_buf();
        let s = vec![
            ("a".to_string(), wt.join("sub")),
            ("x".to_string(), PathBuf::from("/elsewhere")),
        ];
        assert_eq!(session_state(&wt, &[], &|_| None), WorktreeSession::Gone);
        assert_eq!(
            session_state(&wt, &s, &|_| Some("done".into())),
            WorktreeSession::Done
        );
        assert_eq!(session_state(&wt, &s, &|_| None), WorktreeSession::Live);
    }
}
