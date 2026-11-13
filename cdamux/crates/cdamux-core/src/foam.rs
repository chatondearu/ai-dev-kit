//! Read-only `foam/` tree resolution for Fleet (ADR-012).
//!
//! Anchors on git toplevel (or the given path when not a git work tree), then
//! lists markdown under `<project>/foam/` only. Never writes.

use crate::git_toplevel;
use crate::markdown::{render_markdown_terminal, truncate_lines};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Directory name under the project root.
pub const FOAM_DIR: &str = "foam";

/// Soft cap for sidecar list rows (large trees stay usable).
pub const FOAM_LIST_CAP: usize = 500;

/// Hard byte cap before parse/paint (huge ADRs must not freeze the TUI).
pub const FOAM_PREVIEW_MAX_BYTES: usize = 64 * 1024;

/// Visible line cap for the Foam sidecar preview (replaces silent Notes dump).
pub const FOAM_PREVIEW_MAX_LINES: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoamListing {
    /// `<project>/foam` is missing.
    Missing { project: PathBuf },
    /// Directory exists but has no `.md` files (within the walk).
    Empty { project: PathBuf, foam_root: PathBuf },
    /// Relative paths `foam/.../*.md`, sorted, capped at [`FOAM_LIST_CAP`].
    Entries {
        project: PathBuf,
        foam_root: PathBuf,
        paths: Vec<String>,
        truncated: bool,
    },
}

/// Prefer `git rev-parse --show-toplevel`; fall back to the given path.
pub fn resolve_project_root(cwd_or_project: &Path) -> PathBuf {
    git_toplevel(cwd_or_project).unwrap_or_else(|| cwd_or_project.to_path_buf())
}

/// Absolute path to `<project>/foam` when the directory exists.
pub fn resolve_foam_root(cwd_or_project: &Path) -> Option<PathBuf> {
    let project = resolve_project_root(cwd_or_project);
    let foam = project.join(FOAM_DIR);
    foam.is_dir().then_some(foam)
}

/// List markdown under the project's `foam/` tree (deterministic order).
pub fn list_foam(cwd_or_project: &Path) -> FoamListing {
    let project = resolve_project_root(cwd_or_project);
    let foam_root = project.join(FOAM_DIR);
    if !foam_root.is_dir() {
        return FoamListing::Missing { project };
    }
    let mut paths = Vec::new();
    collect_md(&foam_root, &foam_root, &mut paths);
    paths.sort();
    if paths.is_empty() {
        return FoamListing::Empty {
            project,
            foam_root,
        };
    }
    let truncated = paths.len() > FOAM_LIST_CAP;
    if truncated {
        paths.truncate(FOAM_LIST_CAP);
    }
    FoamListing::Entries {
        project,
        foam_root,
        paths,
        truncated,
    }
}

fn collect_md(foam_root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut kids: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    kids.sort();
    for path in kids {
        if path.is_dir() {
            collect_md(foam_root, &path, out);
            continue;
        }
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if !is_md {
            continue;
        }
        let rel = match path.strip_prefix(foam_root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        // Display as foam/<rel> so callers match deep-link paths later (#269).
        let mut display = PathBuf::from(FOAM_DIR);
        display.push(rel);
        out.push(display.to_string_lossy().replace('\\', "/"));
    }
}

/// Absolute path for a `foam/...` listing entry, confined under `<project>/foam`.
fn resolve_listed_foam_file(project: &Path, foam_rel: &str) -> Option<PathBuf> {
    let rest = foam_rel.strip_prefix("foam/").or_else(|| foam_rel.strip_prefix("foam\\"))?;
    if rest.is_empty() || Path::new(rest).is_absolute() {
        return None;
    }
    if rest
        .split(['/', '\\'])
        .any(|seg| seg.is_empty() || seg == "..")
    {
        return None;
    }
    let foam_root = project.join(FOAM_DIR);
    let candidate = foam_root.join(rest);
    let Ok(root) = foam_root.canonicalize() else {
        return None;
    };
    let Ok(path) = candidate.canonicalize() else {
        return None;
    };
    path.starts_with(&root).then_some(path)
}

fn read_capped(path: &Path, max_bytes: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; max_bytes.saturating_add(1)];
    let n = file.read(&mut buf)?;
    let truncated = n > max_bytes;
    buf.truncate(n.min(max_bytes));
    Ok((buf, truncated))
}

/// Render a selected foam markdown path for the sidecar (bounded, read-only).
///
/// `foam_rel` is a listing path such as `foam/index.md`. Non-UTF-8 yields an
/// error line; oversize input/output is truncated with a visible cue.
pub fn preview_foam_markdown(project: &Path, foam_rel: &str) -> String {
    let Some(path) = resolve_listed_foam_file(project, foam_rel) else {
        return format!("(cannot read {foam_rel})");
    };
    let (bytes, truncated_bytes) = match read_capped(&path, FOAM_PREVIEW_MAX_BYTES) {
        Ok(v) => v,
        Err(e) => return format!("(read error: {e})"),
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(s) => s.to_string(),
        Err(err) => {
            if err.valid_up_to() == 0 {
                return "(not valid UTF-8 — skipped)".to_string();
            }
            // Byte cap may cut mid-codepoint; keep the valid prefix.
            String::from_utf8_lossy(&bytes[..err.valid_up_to()]).into_owned()
        }
    };
    let mut rendered = render_markdown_terminal(&text);
    if truncated_bytes {
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str("… (file truncated for preview)");
    }
    truncate_lines(&rendered, FOAM_PREVIEW_MAX_LINES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn init_git(dir: &Path) {
        assert!(Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn resolve_prefers_git_toplevel_over_nested_cwd() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join("nested/cwd")).unwrap();
        init_git(&repo);
        fs::create_dir(repo.join("foam")).unwrap();
        let nested = repo.join("nested/cwd");
        let root = resolve_project_root(&nested);
        assert_eq!(root.canonicalize().unwrap(), repo.canonicalize().unwrap());
        let foam = resolve_foam_root(&nested).unwrap();
        assert_eq!(foam.canonicalize().unwrap(), repo.join("foam").canonicalize().unwrap());
    }

    #[test]
    fn missing_foam_when_dir_absent() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join("nested")).unwrap();
        init_git(&repo);
        match list_foam(&repo.join("nested")) {
            FoamListing::Missing { project } => {
                assert_eq!(
                    project.canonicalize().unwrap(),
                    repo.canonicalize().unwrap()
                );
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn empty_foam_dir() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join("foam")).unwrap();
        init_git(&repo);
        match list_foam(&repo) {
            FoamListing::Empty { .. } => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn lists_markdown_sorted_under_foam() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let foam = repo.join("foam");
        fs::create_dir_all(foam.join("decisions")).unwrap();
        fs::write(foam.join("index.md"), "# moc").unwrap();
        fs::write(foam.join("decisions/ADR-002.md"), "adr").unwrap();
        fs::write(foam.join("decisions/ADR-001.md"), "adr").unwrap();
        fs::write(foam.join("skip.txt"), "no").unwrap();
        fs::write(repo.join("outside.md"), "no").unwrap();
        fs::create_dir_all(repo.join("nested")).unwrap();
        init_git(&repo);

        match list_foam(&repo.join("nested")) {
            FoamListing::Entries {
                paths, truncated, ..
            } => {
                assert!(!truncated);
                assert_eq!(
                    paths,
                    vec![
                        "foam/decisions/ADR-001.md".to_string(),
                        "foam/decisions/ADR-002.md".to_string(),
                        "foam/index.md".to_string(),
                    ]
                );
            }
            other => panic!("expected Entries, got {other:?}"),
        }
    }

    #[test]
    fn non_git_path_uses_given_root() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("not-git");
        fs::create_dir_all(project.join("foam")).unwrap();
        fs::write(project.join("foam/a.md"), "x").unwrap();
        match list_foam(&project) {
            FoamListing::Entries { project: p, paths, .. } => {
                assert_eq!(p, project);
                assert_eq!(paths, vec!["foam/a.md".to_string()]);
            }
            other => panic!("expected Entries, got {other:?}"),
        }
    }

    #[test]
    fn preview_renders_selected_markdown() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        fs::write(
            project.join("foam/index.md"),
            "# Hello\n\n- alpha\n- beta\n",
        )
        .unwrap();
        let out = preview_foam_markdown(&project, "foam/index.md");
        assert!(out.contains("# Hello"), "{out}");
        assert!(out.contains("- alpha"), "{out}");
        assert!(!out.contains("… (truncated)"), "{out}");
    }

    #[test]
    fn preview_truncates_oversize_without_panic() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        let huge: String = (0..FOAM_PREVIEW_MAX_LINES + 40)
            .map(|i| format!("- item {i}\n"))
            .collect();
        fs::write(project.join("foam/big.md"), &huge).unwrap();
        let out = preview_foam_markdown(&project, "foam/big.md");
        assert!(out.contains("… (truncated)"), "{out}");
        assert!(out.lines().count() <= FOAM_PREVIEW_MAX_LINES + 1);
    }

    #[test]
    fn preview_byte_cap_before_parse() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        let mut body = String::from("# Cap\n\n");
        body.push_str(&"x".repeat(FOAM_PREVIEW_MAX_BYTES + 2048));
        fs::write(project.join("foam/huge.md"), &body).unwrap();
        let out = preview_foam_markdown(&project, "foam/huge.md");
        assert!(out.contains("… (file truncated for preview)"), "{out}");
        assert!(out.contains("# Cap"), "{out}");
    }

    #[test]
    fn preview_rejects_non_utf8() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        fs::write(project.join("foam/bin.md"), [0xff, 0xfe, 0xfd]).unwrap();
        let out = preview_foam_markdown(&project, "foam/bin.md");
        assert!(out.contains("UTF-8"), "{out}");
    }

    #[test]
    fn preview_blocks_path_escape() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        fs::write(project.join("secret.md"), "nope").unwrap();
        let out = preview_foam_markdown(&project, "foam/../secret.md");
        assert!(out.contains("cannot read"), "{out}");
    }
}
