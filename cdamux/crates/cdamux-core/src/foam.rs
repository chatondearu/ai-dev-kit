//! Read-only `foam/` tree resolution for Fleet (ADR-012).
//!
//! Anchors on git toplevel (or the given path when not a git work tree), then
//! lists markdown under `<project>/foam/` only. Never writes.
//! Deep-link helpers normalize `foam/...` paths and reject escapes (#269).

use crate::git_toplevel;
use crate::markdown::{render_markdown_terminal, truncate_lines};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

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

/// Trim quotes / trailing punctuation from a path-like token.
/// Does not strip `.` inside extensions (`.md`); only a lone trailing period.
fn trim_path_token(raw: &str) -> &str {
    let s = raw.trim().trim_start_matches(|c: char| {
        matches!(c, '`' | '"' | '\'' | '(' | '[' | '{' | '<')
    });
    let s = s.trim_end_matches(|c: char| {
        matches!(
            c,
            '`' | '"' | '\'' | ')' | ']' | '}' | '>' | ',' | ';' | ':' | '!'
        )
    });
    s.strip_suffix('.').filter(|t| t.contains('/')).unwrap_or(s)
}

fn has_parent_dir(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

fn foam_display(rel_under_foam: &Path) -> String {
    let mut display = PathBuf::from(FOAM_DIR);
    display.push(rel_under_foam);
    display.to_string_lossy().replace('\\', "/")
}

/// Normalize `raw` to a `foam/...` path under the anchored project, or `None`.
///
/// Accepts repo-relative `foam/...` and absolute paths under `<project>/foam/`.
/// Rejects `..` escapes (lexical and after canonicalize when the path exists).
/// Non-foam paths return `None` (caller must ignore).
pub fn normalize_foam_path(raw: &str, cwd_or_project: &Path) -> Option<String> {
    let token = trim_path_token(raw);
    if token.is_empty() {
        return None;
    }
    let project = resolve_project_root(cwd_or_project);
    let foam_root = project.join(FOAM_DIR);
    let candidate = Path::new(token);
    if has_parent_dir(candidate) {
        return None;
    }

    let abs = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        let rel = token.trim_start_matches("./");
        if rel == FOAM_DIR {
            return None;
        }
        if let Some(rest) = rel.strip_prefix("foam/") {
            if rest.is_empty() || has_parent_dir(Path::new(rest)) {
                return None;
            }
            foam_root.join(rest)
        } else if let Some(idx) = token.find("/foam/") {
            // e.g. relative worktree prefix …/repo/foam/index.md without leading /
            return normalize_foam_path(&token[idx + 1..], &project);
        } else {
            return None;
        }
    };

    if has_parent_dir(&abs) {
        return None;
    }

    // Existing paths: canonicalize so symlink/`..` cannot escape foam/.
    if abs.exists() && foam_root.is_dir() {
        if let (Ok(a), Ok(f)) = (abs.canonicalize(), foam_root.canonicalize()) {
            let rel = a.strip_prefix(&f).ok()?;
            if rel.as_os_str().is_empty() || has_parent_dir(rel) {
                return None;
            }
            return Some(foam_display(rel));
        }
    }

    let rel = abs.strip_prefix(&foam_root).ok()?;
    if rel.as_os_str().is_empty() || has_parent_dir(rel) {
        return None;
    }
    Some(foam_display(rel))
}

/// Index of `foam_rel` in a listing, if present.
pub fn find_foam_entry(paths: &[String], foam_rel: &str) -> Option<usize> {
    paths.iter().position(|p| p == foam_rel)
}

fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '~')
}

/// Last `foam/...` (or `…/foam/...`) path-like token in `text`, if any.
///
/// Fleet deep-links from pane preview capture (#269): agents often mention the
/// file they just touched last in the buffer.
pub fn extract_foam_path_from_text(text: &str) -> Option<String> {
    let mut last: Option<String> = None;
    let mut i = 0usize;
    while i < text.len() {
        let rest = &text[i..];
        let Some(rel) = rest.find("foam/") else {
            break;
        };
        let abs_at = i + rel;
        // Walk back over absolute / relative path prefix before foam/.
        let mut start = abs_at;
        while start > 0 {
            let Some((prev_idx, prev)) = text[..start].char_indices().next_back() else {
                break;
            };
            if is_path_char(prev) || prev == ':' {
                start = prev_idx;
            } else {
                break;
            }
        }
        let mut end = abs_at + "foam/".len();
        while end < text.len() {
            let ch = text[end..].chars().next().unwrap();
            if is_path_char(ch) {
                end += ch.len_utf8();
            } else {
                break;
            }
        }
        let token = trim_path_token(&text[start..end]);
        if token.contains("foam/") && !token.ends_with("foam/") {
            last = Some(token.to_string());
        }
        i = abs_at + "foam/".len();
    }
    last
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

/// Max files remembered for the Foam "back" key.
const FOAM_HISTORY_MAX: usize = 50;

/// One visible row of the Foam tree (directory or markdown file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoamRow {
    /// `foam/...` path (directory rows have no `.md` suffix).
    pub path: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
}

impl FoamRow {
    /// Last path segment (`ADR-001.md`, `decisions`).
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// Collapsible tree over a [`list_foam`] listing: expand state, cursor, back history.
///
/// Pure in-memory model (no I/O); the Fleet Foam tab drives it from keys and clicks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoamTree {
    paths: Vec<String>,
    collapsed: HashSet<String>,
    cursor: String,
    history: Vec<String>,
}

impl FoamTree {
    /// All directories start expanded; the cursor starts on the first row.
    pub fn new(paths: Vec<String>) -> Self {
        let mut tree = Self {
            paths,
            collapsed: HashSet::new(),
            cursor: String::new(),
            history: Vec::new(),
        };
        tree.cursor = tree.rows().first().map(|r| r.path.clone()).unwrap_or_default();
        tree
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Visible rows in listing order; children of collapsed directories are skipped.
    pub fn rows(&self) -> Vec<FoamRow> {
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        for p in &self.paths {
            let rel = p.strip_prefix("foam/").unwrap_or(p);
            let segs: Vec<&str> = rel.split('/').collect();
            let mut dir = String::from(FOAM_DIR);
            let mut visible = true;
            for (depth, seg) in segs[..segs.len() - 1].iter().enumerate() {
                dir.push('/');
                dir.push_str(seg);
                let expanded = !self.collapsed.contains(&dir);
                if seen.insert(dir.clone()) {
                    rows.push(FoamRow {
                        path: dir.clone(),
                        depth,
                        is_dir: true,
                        expanded,
                    });
                }
                if !expanded {
                    visible = false;
                    break;
                }
            }
            if visible {
                rows.push(FoamRow {
                    path: p.clone(),
                    depth: segs.len() - 1,
                    is_dir: false,
                    expanded: false,
                });
            }
        }
        rows
    }

    /// Index of the cursor in `rows` (first row when the cursor row is hidden).
    pub fn cursor_index(&self, rows: &[FoamRow]) -> usize {
        rows.iter().position(|r| r.path == self.cursor).unwrap_or(0)
    }

    /// File under the cursor, if the cursor is on a file row.
    pub fn selected_file(&self) -> Option<&str> {
        self.paths.iter().find(|p| **p == self.cursor).map(String::as_str)
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let rows = self.rows();
        let idx = self.cursor_index(&rows).saturating_add_signed(delta);
        if let Some(r) = rows.get(idx.min(rows.len().saturating_sub(1))) {
            self.cursor = r.path.clone();
        }
    }

    /// Put the cursor on visible row `idx`; a directory row also toggles (click).
    pub fn click(&mut self, idx: usize) {
        let rows = self.rows();
        let Some(row) = rows.get(idx) else { return };
        self.cursor = row.path.clone();
        if row.is_dir {
            self.toggle_cursor();
        }
    }

    /// Flip the directory under the cursor; no-op on files.
    pub fn toggle_cursor(&mut self) {
        if self.paths.contains(&self.cursor) {
            return;
        }
        if !self.collapsed.remove(&self.cursor) {
            self.collapsed.insert(self.cursor.clone());
        }
    }

    /// Right: expand the directory under the cursor.
    pub fn expand_cursor(&mut self) {
        self.collapsed.remove(&self.cursor);
    }

    /// Left: collapse an expanded directory, otherwise jump to the parent row.
    pub fn collapse_or_parent(&mut self) {
        let rows = self.rows();
        let Some(row) = rows.get(self.cursor_index(&rows)) else {
            return;
        };
        if row.is_dir && row.expanded {
            self.collapsed.insert(row.path.clone());
        } else if let Some((parent, _)) = row.path.rsplit_once('/') {
            if parent != FOAM_DIR {
                self.cursor = parent.to_string();
            }
        }
    }

    /// Expand every ancestor of `path` and put the cursor on it.
    pub fn select_path(&mut self, path: &str) {
        let mut dir = String::from(FOAM_DIR);
        let rel = path.strip_prefix("foam/").unwrap_or(path);
        let segs: Vec<&str> = rel.split('/').collect();
        for seg in &segs[..segs.len() - 1] {
            dir.push('/');
            dir.push_str(seg);
            self.collapsed.remove(&dir);
        }
        self.cursor = path.to_string();
    }

    /// Jump to a wikilink target, remembering the current file for [`Self::back`].
    pub fn follow(&mut self, target: &str) {
        if let Some(cur) = self.selected_file().map(str::to_string) {
            if self.history.last() != Some(&cur) {
                self.history.push(cur);
                if self.history.len() > FOAM_HISTORY_MAX {
                    self.history.remove(0);
                }
            }
        }
        self.select_path(target);
    }

    /// Return to the file before the last followed link.
    pub fn back(&mut self) -> bool {
        let Some(prev) = self.history.pop() else {
            return false;
        };
        self.select_path(&prev);
        true
    }
}

/// A `[[target]]` / `[[target|label]]` reference found in markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub label: String,
}

/// Wikilinks in `text`, in order, deduplicated by target. `[[t#heading|l]]` keeps `t`.
pub fn extract_wikilinks(text: &str) -> Vec<WikiLink> {
    let mut out: Vec<WikiLink> = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(open) = rest.find("[[") {
            let after = &rest[open + 2..];
            let Some(close) = after.find("]]") else { break };
            let inner = &after[..close];
            rest = &after[close + 2..];
            let (raw, label) = inner.split_once('|').unwrap_or((inner, inner));
            let target = raw.split('#').next().unwrap_or("").trim();
            if target.is_empty() || out.iter().any(|l| l.target == target) {
                continue;
            }
            out.push(WikiLink {
                target: target.to_string(),
                label: label.trim().to_string(),
            });
        }
    }
    out
}

/// Resolve a wikilink target against a listing: exact `foam/<target>.md` path,
/// then the first file whose stem matches the last segment (case-insensitive).
pub fn resolve_wikilink(paths: &[String], target: &str) -> Option<String> {
    let t = target.trim().trim_start_matches("./");
    let t = t.strip_suffix(".md").unwrap_or(t);
    let t = t.strip_prefix("foam/").unwrap_or(t);
    if t.is_empty() {
        return None;
    }
    let exact = format!("{FOAM_DIR}/{t}.md");
    if let Some(p) = paths.iter().find(|p| **p == exact) {
        return Some(p.clone());
    }
    let stem = t.rsplit('/').next().unwrap_or(t);
    paths
        .iter()
        .find(|p| {
            p.rsplit('/')
                .next()
                .and_then(|f| f.strip_suffix(".md"))
                .is_some_and(|s| s.eq_ignore_ascii_case(stem))
        })
        .cloned()
}

/// Wikilinks of a listed foam file (bounded read, same caps as the preview).
pub fn foam_file_wikilinks(project: &Path, foam_rel: &str) -> Vec<WikiLink> {
    let Some(path) = resolve_listed_foam_file(project, foam_rel) else {
        return Vec::new();
    };
    let Ok((bytes, _)) = read_capped(&path, FOAM_PREVIEW_MAX_BYTES) else {
        return Vec::new();
    };
    extract_wikilinks(&String::from_utf8_lossy(&bytes))
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
    #[test]
    fn normalize_accepts_relative_and_absolute_foam_paths() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam/decisions")).unwrap();
        fs::write(project.join("foam/decisions/ADR-001.md"), "a").unwrap();
        init_git(&project);

        assert_eq!(
            normalize_foam_path("foam/decisions/ADR-001.md", &project).as_deref(),
            Some("foam/decisions/ADR-001.md")
        );
        assert_eq!(
            normalize_foam_path("./foam/decisions/ADR-001.md", &project).as_deref(),
            Some("foam/decisions/ADR-001.md")
        );
        let abs = project.join("foam/decisions/ADR-001.md");
        assert_eq!(
            normalize_foam_path(abs.to_str().unwrap(), &project).as_deref(),
            Some("foam/decisions/ADR-001.md")
        );
        assert_eq!(
            normalize_foam_path("`foam/decisions/ADR-001.md`", &project).as_deref(),
            Some("foam/decisions/ADR-001.md")
        );
    }
    #[test]
    fn normalize_rejects_escape_and_non_foam() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        fs::write(project.join("secret.md"), "no").unwrap();
        init_git(&project);

        assert_eq!(normalize_foam_path("prd/PRD-1.md", &project), None);
        assert_eq!(normalize_foam_path("foam/../secret.md", &project), None);
        assert_eq!(
            normalize_foam_path(
                project.join("foam/../secret.md").to_str().unwrap(),
                &project
            ),
            None
        );
        assert_eq!(normalize_foam_path("foam", &project), None);
    }
    #[test]
    fn extract_prefers_last_foam_token() {
        let text = "see foam/index.md then foam/decisions/ADR-004.md.";
        assert_eq!(
            extract_foam_path_from_text(text).as_deref(),
            Some("foam/decisions/ADR-004.md")
        );
        assert_eq!(extract_foam_path_from_text("no foam here"), None);
        assert_eq!(
            extract_foam_path_from_text("touch `foam/a.md` please").as_deref(),
            Some("foam/a.md")
        );
    }
    #[test]
    fn find_entry_index() {
        let paths = vec![
            "foam/a.md".to_string(),
            "foam/b.md".to_string(),
        ];
        assert_eq!(find_foam_entry(&paths, "foam/b.md"), Some(1));
        assert_eq!(find_foam_entry(&paths, "foam/missing.md"), None);
    }

    fn tree(paths: &[&str]) -> FoamTree {
        FoamTree::new(paths.iter().map(|p| p.to_string()).collect())
    }

    fn row_names(t: &FoamTree) -> Vec<String> {
        t.rows()
            .iter()
            .map(|r| format!("{}{}{}", " ".repeat(r.depth), r.name(), if r.is_dir { "/" } else { "" }))
            .collect()
    }

    #[test]
    fn tree_rows_nest_dirs_and_collapse() {
        let mut t = tree(&[
            "foam/decisions/ADR-001.md",
            "foam/decisions/ADR-002.md",
            "foam/index.md",
        ]);
        assert_eq!(
            row_names(&t),
            ["decisions/", " ADR-001.md", " ADR-002.md", "index.md"]
        );
        t.click(0);
        assert_eq!(row_names(&t), ["decisions/", "index.md"]);
        t.click(0);
        assert_eq!(row_names(&t).len(), 4);
    }

    #[test]
    fn tree_cursor_moves_clamps_and_jumps_to_parent() {
        let mut t = tree(&["foam/a/b/x.md", "foam/y.md"]);
        t.move_cursor(-5);
        assert_eq!(t.cursor_index(&t.rows()), 0);
        t.move_cursor(2);
        assert_eq!(t.selected_file(), Some("foam/a/b/x.md"));
        t.collapse_or_parent();
        assert_eq!(t.cursor, "foam/a/b");
        // Expanded dir collapses in place.
        t.collapse_or_parent();
        assert_eq!(row_names(&t), ["a/", " b/", "y.md"]);
    }

    #[test]
    fn select_path_expands_ancestors() {
        let mut t = tree(&["foam/a/b/x.md", "foam/y.md"]);
        t.click(0);
        assert_eq!(row_names(&t), ["a/", "y.md"]);
        t.select_path("foam/a/b/x.md");
        assert_eq!(t.selected_file(), Some("foam/a/b/x.md"));
        assert_eq!(row_names(&t).len(), 4);
    }

    #[test]
    fn follow_and_back_use_history() {
        let mut t = tree(&["foam/a.md", "foam/b.md", "foam/c.md"]);
        t.select_path("foam/a.md");
        t.follow("foam/b.md");
        t.follow("foam/c.md");
        assert_eq!(t.selected_file(), Some("foam/c.md"));
        assert!(t.back());
        assert_eq!(t.selected_file(), Some("foam/b.md"));
        assert!(t.back());
        assert_eq!(t.selected_file(), Some("foam/a.md"));
        assert!(!t.back());
    }

    #[test]
    fn wikilinks_parse_label_anchor_and_dedupe() {
        let links = extract_wikilinks(
            "see [[ADR-001]] and [[foam/decisions/ADR-002|two]], [[ADR-001#why]] [[ ]] [[open",
        );
        let got: Vec<(&str, &str)> = links.iter().map(|l| (l.target.as_str(), l.label.as_str())).collect();
        assert_eq!(got, [("ADR-001", "ADR-001"), ("foam/decisions/ADR-002", "two")]);
    }

    #[test]
    fn wikilink_resolves_path_then_stem() {
        let paths: Vec<String> = ["foam/decisions/ADR-001.md", "foam/notes/adr-001.md", "foam/index.md"]
            .map(String::from)
            .to_vec();
        let r = |t| resolve_wikilink(&paths, t);
        assert_eq!(r("notes/adr-001").as_deref(), Some("foam/notes/adr-001.md"));
        assert_eq!(r("foam/index.md").as_deref(), Some("foam/index.md"));
        // Ambiguous stem: first in listing order wins.
        assert_eq!(r("ADR-001").as_deref(), Some("foam/decisions/ADR-001.md"));
        assert_eq!(r("../foam/decisions/ADR-001").as_deref(), Some("foam/decisions/ADR-001.md"));
        assert_eq!(r("missing"), None);
    }

    #[test]
    fn file_wikilinks_read_from_disk_and_stay_confined() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join("foam")).unwrap();
        fs::write(project.join("foam/a.md"), "[[b]]").unwrap();
        fs::write(project.join("secret.md"), "[[leak]]").unwrap();
        assert_eq!(foam_file_wikilinks(&project, "foam/a.md").len(), 1);
        assert!(foam_file_wikilinks(&project, "foam/../secret.md").is_empty());
    }
}
