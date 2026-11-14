//! Fleet Foam tab: collapsible `foam/` tree, preview and `[[wikilinks]]` (ADR-012).
//!
//! Read-only. The tab body is plain text (tree window, preview, links); the tree
//! rows are painted on contiguous lines so a click maps back to a row.

use cdamux_core::{
    foam_file_wikilinks, list_foam, normalize_foam_path, preview_foam_markdown,
    resolve_project_root, resolve_wikilink, FoamListing, FoamTree, WikiLink, FOAM_LIST_CAP,
};
use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use std::path::{Path, PathBuf};

/// Tree rows painted around the cursor for a body `height`: about half of it,
/// so the preview below stays on screen even in a short pane.
fn foam_list_rows(height: u16) -> usize {
    (height as usize / 2).saturating_sub(2).clamp(3, 12)
}

/// Footer while the Foam tree has focus.
pub(crate) const FOAM_FOOTER: &str =
    "foam: j/k ↑↓ move · h/l ←→ fold · Enter open/follow · [ ] link · b back · click row · Esc leave";

/// Max wikilinks kept per file, and painted at once (window around the selection).
const FOAM_LINKS_MAX: usize = 20;
const FOAM_LINKS_SHOWN: usize = 4;

/// Window `[start, end)` around `sel`, biased high so the preview stays on screen.
fn foam_list_window(sel: usize, len: usize, max_rows: usize) -> (usize, usize) {
    if len == 0 || max_rows == 0 {
        return (0, 0);
    }
    if len <= max_rows {
        return (0, len);
    }
    // One row above the cursor when possible.
    let mut start = sel.saturating_sub(1.min(max_rows.saturating_sub(1)));
    if start + max_rows > len {
        start = len - max_rows;
    }
    (start, start + max_rows)
}

/// Result of opening a foam path from a deep-link (#269).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FoamOpen {
    /// Non-foam path — leave the current tab alone.
    Ignored,
    /// The cursor moved onto an existing file.
    Focus { path: String },
    /// Foam path normalized but absent from the listing (banner shown).
    Missing { path: String },
}

pub(crate) struct FoamState {
    /// Rendered tab body.
    pub(crate) text: String,
    /// Shown instead of the tree when `foam/` is missing, empty or no session.
    base: String,
    tree: Option<FoamTree>,
    project: PathBuf,
    truncated: bool,
    /// Wikilinks of the file under the cursor with their resolved target.
    links: Vec<(WikiLink, Option<String>)>,
    links_for: Option<String>,
    link_sel: usize,
    notice: Option<String>,
    /// Size of the sidecar body, set by the draw loop (lines are clipped to `width`
    /// so one text line is one screen row and clicks map back to rows).
    width: usize,
    height: u16,
    /// First text line of the painted tree rows and the visible-row window.
    list_line0: usize,
    window: (usize, usize),
}

impl FoamState {
    /// No tree: show `message` only.
    pub(crate) fn message(message: &str) -> Self {
        Self {
            text: message.to_string(),
            base: message.to_string(),
            tree: None,
            project: PathBuf::new(),
            truncated: false,
            links: Vec::new(),
            links_for: None,
            link_sel: 0,
            notice: None,
            width: usize::MAX,
            height: 24,
            list_line0: 0,
            window: (0, 0),
        }
    }

    /// List `foam/**/*.md` for the session cwd (read-only, ADR-012).
    pub(crate) fn load(cwd: &str) -> Self {
        if cwd.is_empty() {
            return Self::message("Select a session with a working directory.");
        }
        match list_foam(Path::new(cwd)) {
            FoamListing::Missing { project } => {
                let mut s = Self::message(&format!(
                    "No foam/ under {}\n\nCreate a foam/ tree (ADRs, MOC) to browse it here.\nCLI notes remain: `cdamux notes edit`.",
                    project.display()
                ));
                s.project = project;
                s
            }
            FoamListing::Empty { project, .. } => {
                let mut s = Self::message(&format!(
                    "foam/ is empty under {}\n\nAdd markdown under foam/ to list it here.",
                    project.display()
                ));
                s.project = project;
                s
            }
            FoamListing::Entries {
                project,
                paths,
                truncated,
                ..
            } => {
                let mut s = Self::message("");
                s.project = project;
                s.truncated = truncated;
                s.tree = Some(FoamTree::new(paths));
                s.render();
                s
            }
        }
    }

    /// Track the sidecar body size; true when the body had to be repainted.
    pub(crate) fn set_size(&mut self, width: u16, height: u16) -> bool {
        if self.height == height && self.width == width as usize {
            return false;
        }
        self.height = height;
        self.width = width as usize;
        self.tree.is_some() && {
            self.render();
            true
        }
    }

    pub(crate) fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    #[cfg(test)]
    pub(crate) fn selected_file(&self) -> Option<&str> {
        self.tree.as_ref()?.selected_file()
    }

    fn clip(&self, line: String) -> String {
        if line.chars().count() <= self.width {
            return line;
        }
        let keep = self.width.saturating_sub(1);
        line.chars().take(keep).chain(std::iter::once('…')).collect()
    }

    /// Re-render the body from the tree state.
    fn render(&mut self) {
        let Some(tree) = self.tree.as_ref() else {
            self.text = match &self.notice {
                Some(n) => format!("{n}\n\n{}", self.base),
                None => self.base.clone(),
            };
            return;
        };
        let rows = tree.rows();
        let cursor = tree.cursor_index(&rows);
        let file = tree.selected_file().map(str::to_string);
        if self.links_for != file {
            self.links = file
                .as_deref()
                .map(|f| {
                    foam_file_wikilinks(&self.project, f)
                        .into_iter()
                        .take(FOAM_LINKS_MAX)
                        .map(|l| {
                            let target = resolve_wikilink(tree.paths(), &l.target);
                            (l, target)
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.links_for = file.clone();
            self.link_sel = 0;
        }

        let (start, end) = foam_list_window(cursor, rows.len(), foam_list_rows(self.height));
        let mut lines = vec![self.clip(format!("foam/ · {}", self.project.display()))];
        if let Some(n) = &self.notice {
            lines.extend(n.lines().map(|l| self.clip(l.to_string())));
            lines.push(String::new());
        }
        if start > 0 {
            lines.push(format!("… ({start} more above)"));
        }
        self.list_line0 = lines.len();
        self.window = (start, end);
        for (i, row) in rows.iter().enumerate().take(end).skip(start) {
            let mark = if i == cursor { "▸" } else { " " };
            let indent = "  ".repeat(row.depth);
            let label = if row.is_dir {
                format!("[{}] {}/", if row.expanded { '-' } else { '+' }, row.name())
            } else {
                format!("    {}", row.name())
            };
            lines.push(self.clip(format!("{mark} {indent}{label}")));
        }
        if end < rows.len() {
            lines.push(format!("… ({} more below)", rows.len() - end));
        }
        if self.truncated {
            lines.push(format!("… truncated at {FOAM_LIST_CAP} files"));
        }
        if file.is_some() && !self.links.is_empty() {
            // Before the preview: a long note must not push the links off-screen.
            lines.push("── links ──".to_string());
            let (ls, le) = foam_list_window(self.link_sel, self.links.len(), FOAM_LINKS_SHOWN);
            for (i, (link, target)) in self.links.iter().enumerate().take(le).skip(ls) {
                let mark = if i == self.link_sel { "▸" } else { " " };
                let dest = target.as_deref().unwrap_or("(unresolved)");
                lines.push(self.clip(format!("{mark} [[{}]] → {dest}", link.label)));
            }
            if le < self.links.len() {
                lines.push(format!("… ({} more links)", self.links.len() - le));
            }
        }
        if let Some(f) = &file {
            lines.push("── preview ──".to_string());
            lines.push(preview_foam_markdown(&self.project, f));
        }
        self.text = lines.join("\n");
    }

    /// Handle a Foam-focus key; false when the key is not a Foam key.
    pub(crate) fn on_key(&mut self, code: KeyCode) -> bool {
        let Some(tree) = self.tree.as_mut() else {
            return false;
        };
        // Cleared up-front; the `_` arm below repaints when it was showing.
        let had_notice = self.notice.take();
        match code {
            KeyCode::Down | KeyCode::Char('j') => tree.move_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => tree.move_cursor(-1),
            KeyCode::Left | KeyCode::Char('h') => tree.collapse_or_parent(),
            KeyCode::Right | KeyCode::Char('l') => tree.expand_cursor(),
            KeyCode::Char('[') => self.link_sel = self.link_sel.saturating_sub(1),
            KeyCode::Char(']') => {
                self.link_sel = (self.link_sel + 1).min(self.links.len().saturating_sub(1));
            }
            KeyCode::Enter => {
                if tree.selected_file().is_none() {
                    tree.toggle_cursor();
                } else {
                    match self.links.get(self.link_sel) {
                        Some((_, Some(target))) => tree.follow(target),
                        Some((link, None)) => {
                            self.notice = Some(format!("[[{}]] does not resolve under foam/", link.target));
                        }
                        None => {}
                    }
                }
            }
            KeyCode::Backspace | KeyCode::Char('b') => {
                if !tree.back() {
                    self.notice = Some("no previous file".to_string());
                }
            }
            _ => {
                if had_notice.is_some() {
                    self.render();
                }
                return false;
            }
        }
        self.render();
        true
    }

    /// Click on text line `line` of the body; true when it hit a tree row.
    pub(crate) fn click_line(&mut self, line: usize) -> bool {
        let (start, end) = self.window;
        let Some(tree) = self.tree.as_mut() else {
            return false;
        };
        if line < self.list_line0 || line >= self.list_line0 + (end - start) {
            return false;
        }
        self.notice = None;
        tree.click(start + line - self.list_line0);
        self.render();
        true
    }

    /// In-app Foam deep-link: normalize `raw`, move the cursor or show a banner.
    pub(crate) fn open_path(&mut self, raw: &str) -> FoamOpen {
        if self.project.as_os_str().is_empty() {
            return FoamOpen::Ignored;
        }
        let project = resolve_project_root(&self.project);
        let Some(path) = normalize_foam_path(raw, &project) else {
            return FoamOpen::Ignored;
        };
        match self.tree.as_mut() {
            Some(tree) if tree.paths().contains(&path) => {
                tree.select_path(&path);
                self.notice = None;
                self.render();
                FoamOpen::Focus { path }
            }
            _ => {
                self.notice = Some(format!(
                    "{path} not found under {}\n\nDeep-link target missing from foam/.",
                    project.display()
                ));
                self.render();
                FoamOpen::Missing { path }
            }
        }
    }

    /// Apply a preview-capture deep-link once per distinct foam path (#269).
    ///
    /// Context source: pane preview text (last `foam/...` token). Returns true
    /// when the state changed. Callers invoke only while the Foam tab is visible
    /// (#294) — Preview-first open must not walk foam for deep-link.
    pub(crate) fn apply_deep_link_from_preview(
        &mut self,
        preview: &str,
        applied: &mut Option<String>,
    ) -> bool {
        let Some(raw) = cdamux_core::extract_foam_path_from_text(preview) else {
            return false;
        };
        if self.project.as_os_str().is_empty() {
            return false;
        }
        // Probe first: a repeated path must not reset the operator's cursor.
        let Some(path) = normalize_foam_path(&raw, &resolve_project_root(&self.project)) else {
            return false;
        };
        if applied.as_deref() == Some(path.as_str()) {
            return false;
        }
        match self.open_path(&raw) {
            FoamOpen::Ignored => false,
            FoamOpen::Focus { path } | FoamOpen::Missing { path } => {
                *applied = Some(path);
                true
            }
        }
    }
}

/// Body text line under a mouse position inside the sidecar body `area`.
pub(crate) fn foam_click_line(area: Rect, column: u16, row: u16) -> Option<usize> {
    let inside = column >= area.x
        && column < area.x + area.width
        && row >= area.y
        && row < area.y + area.height;
    inside.then(|| (row - area.y) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_render::render_fleet_foam;
    use std::fs;
    use tempfile::tempdir;

    fn fixture(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
        let tmp = tempdir().unwrap();
        for (rel, body) in files {
            let p = tmp.path().join("foam").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        let cwd = tmp.path().to_str().unwrap().to_string();
        (tmp, cwd)
    }

    #[test]
    fn tree_renders_nested_rows_cursor_and_preview() {
        let (_t, cwd) = fixture(&[
            ("index.md", "# moc"),
            ("decisions/ADR-001.md", "# adr"),
        ]);
        let st = FoamState::load(&cwd);
        // Cursor starts on the first row: the directory (no preview yet).
        assert!(st.text.contains("▸ [-] decisions/"), "{}", st.text);
        assert!(st.text.contains("      ADR-001.md"), "{}", st.text);
        assert!(st.text.contains("index.md"), "{}", st.text);
        assert!(!st.text.contains("── preview ──"), "{}", st.text);
    }

    #[test]
    fn keys_move_collapse_and_preview_file() {
        let (_t, cwd) = fixture(&[
            ("index.md", "# moc"),
            ("decisions/ADR-001.md", "# Preview me\n\n- item\n"),
        ]);
        let mut st = FoamState::load(&cwd);
        assert!(st.on_key(KeyCode::Down));
        assert_eq!(st.selected_file(), Some("foam/decisions/ADR-001.md"));
        assert!(st.text.contains("── preview ──"), "{}", st.text);
        assert!(st.text.contains("# Preview me"), "{}", st.text);
        // Left on a file jumps to its dir; Left again collapses it.
        st.on_key(KeyCode::Left);
        st.on_key(KeyCode::Left);
        assert!(st.text.contains("▸ [+] decisions/"), "{}", st.text);
        assert!(!st.text.contains("ADR-001.md"), "{}", st.text);
        // Right expands, Enter toggles.
        st.on_key(KeyCode::Right);
        assert!(st.text.contains("ADR-001.md"), "{}", st.text);
        st.on_key(KeyCode::Enter);
        assert!(st.text.contains("[+] decisions/"), "{}", st.text);
        assert!(!st.on_key(KeyCode::Char('z')));
    }

    #[test]
    fn click_selects_row_and_toggles_dirs() {
        let (_t, cwd) = fixture(&[("a/x.md", "# x"), ("y.md", "# y")]);
        let mut st = FoamState::load(&cwd);
        let line_of = |st: &FoamState, needle: &str| {
            st.text.lines().position(|l| l.contains(needle)).unwrap()
        };
        let y = line_of(&st, "y.md");
        assert!(st.click_line(y));
        assert_eq!(st.selected_file(), Some("foam/y.md"));
        assert!(!st.click_line(line_of(&st, "── preview ──")));
        // Directory row: click collapses it.
        let a = line_of(&st, "[-] a/");
        assert!(st.click_line(a));
        assert!(st.text.contains("[+] a/"), "{}", st.text);
        assert!(!st.text.contains("x.md"), "{}", st.text);
        // Header / preview lines are not rows.
        assert!(!st.click_line(0));
    }

    #[test]
    fn tree_snapshot_expanded_and_collapsed_dirs() {
        let (_t, cwd) = fixture(&[
            ("decisions/ADR-001.md", "# one"),
            ("decisions/ADR-002.md", "# two"),
            ("features/list.md", "[[ADR-001]]"),
            ("index.md", "# moc"),
        ]);
        let mut st = FoamState::load(&cwd);
        // Click collapses decisions/ (line 1: right under the header), then walk to the file.
        assert!(st.click_line(1));
        st.on_key(KeyCode::Char('j'));
        st.on_key(KeyCode::Char('j'));
        assert_eq!(st.selected_file(), Some("foam/features/list.md"));
        // Header carries the temp path: pin it for a stable snapshot.
        let text = st.text.replacen(&format!("{}", st.project.display()), "<project>", 1);
        crate::test_render::assert_snapshot(
            "foam-tree-80x24",
            &render_fleet_foam(80, 24, &text),
        );
    }

    #[test]
    fn long_note_keeps_links_above_preview_and_rows_clipped_to_width() {
        let body = format!("[[b]]\n{}", "line\n".repeat(100));
        let (_t, cwd) = fixture(&[("a.md", &body), ("b.md", "# b")]);
        let mut st = FoamState::load(&cwd);
        st.set_size(30, 12);
        assert_eq!(st.selected_file(), Some("foam/a.md"));
        let at = |needle: &str| st.text.lines().position(|l| l.contains(needle)).unwrap();
        assert!(at("── links ──") < at("── preview ──"), "{}", st.text);
        // Header / rows never exceed the width, so no wrap shifts click rows.
        assert!(st.text.lines().take(4).all(|l| l.chars().count() <= 30), "{}", st.text);
    }

    #[test]
    fn click_line_maps_inside_area_only() {
        let area = Rect::new(10, 5, 20, 8);
        assert_eq!(foam_click_line(area, 10, 5), Some(0));
        assert_eq!(foam_click_line(area, 29, 12), Some(7));
        assert_eq!(foam_click_line(area, 30, 5), None);
        assert_eq!(foam_click_line(area, 9, 5), None);
        assert_eq!(foam_click_line(area, 10, 13), None);
    }

    #[test]
    fn enter_follows_wikilink_and_back_returns() {
        let (_t, cwd) = fixture(&[
            ("a.md", "go [[b]] or [[ghost]]"),
            ("sub/b.md", "# Bee"),
        ]);
        let mut st = FoamState::load(&cwd);
        // Rows: a.md, sub/, sub/b.md — cursor on a.md after skipping nothing.
        st.on_key(KeyCode::Char('j'));
        st.on_key(KeyCode::Char('k'));
        assert_eq!(st.selected_file(), Some("foam/a.md"));
        assert!(st.text.contains("[[b]] → foam/sub/b.md"), "{}", st.text);
        assert!(st.text.contains("[[ghost]] → (unresolved)"), "{}", st.text);
        // Unresolved link: stay put, banner.
        st.on_key(KeyCode::Char(']'));
        st.on_key(KeyCode::Enter);
        assert_eq!(st.selected_file(), Some("foam/a.md"));
        assert!(st.text.contains("does not resolve"), "{}", st.text);
        // Resolved link: follow, then back.
        st.on_key(KeyCode::Char('['));
        st.on_key(KeyCode::Enter);
        assert_eq!(st.selected_file(), Some("foam/sub/b.md"));
        assert!(st.text.contains("# Bee"), "{}", st.text);
        st.on_key(KeyCode::Char('b'));
        assert_eq!(st.selected_file(), Some("foam/a.md"));
        st.on_key(KeyCode::Backspace);
        assert!(st.text.contains("no previous file"), "{}", st.text);
    }

    #[test]
    fn missing_and_empty_states() {
        let tmp = tempdir().unwrap();
        let st = FoamState::load(tmp.path().to_str().unwrap());
        assert!(!st.has_tree());
        assert!(st.text.contains("No foam/"), "{}", st.text);
        fs::create_dir_all(tmp.path().join("foam")).unwrap();
        let st = FoamState::load(tmp.path().to_str().unwrap());
        assert!(st.text.contains("foam/ is empty"), "{}", st.text);
        assert!(FoamState::load("").text.contains("Select a session"));
    }

    #[test]
    fn deep_link_focuses_row_expanding_collapsed_dirs() {
        let (_t, cwd) = fixture(&[("index.md", "# moc"), ("decisions/ADR-001.md", "a")]);
        let mut st = FoamState::load(&cwd);
        st.on_key(KeyCode::Enter); // collapse decisions/
        assert!(!st.text.contains("ADR-001.md"));
        assert_eq!(
            st.open_path("foam/decisions/ADR-001.md"),
            FoamOpen::Focus { path: "foam/decisions/ADR-001.md".into() }
        );
        assert_eq!(st.selected_file(), Some("foam/decisions/ADR-001.md"));
        assert!(st.text.lines().any(|l| l.starts_with('▸') && l.ends_with("ADR-001.md")), "{}", st.text);
    }

    #[test]
    fn deep_link_missing_keeps_listing_and_ignores_non_foam() {
        let (_t, cwd) = fixture(&[("index.md", "x")]);
        let mut st = FoamState::load(&cwd);
        assert_eq!(st.open_path("prd/PRD-1.md"), FoamOpen::Ignored);
        assert_eq!(
            st.open_path("foam/nope.md"),
            FoamOpen::Missing { path: "foam/nope.md".into() }
        );
        assert!(st.text.contains("not found"), "{}", st.text);
        assert!(st.text.contains("index.md"), "banner + list: {}", st.text);
        // Any Foam key clears the banner.
        st.on_key(KeyCode::Down);
        assert!(!st.text.contains("not found"), "{}", st.text);
    }

    #[test]
    fn preview_deep_link_selects_once_and_keeps_cursor_after() {
        let (_t, cwd) = fixture(&[("index.md", "# moc"), ("adr.md", "a")]);
        let mut st = FoamState::load(&cwd);
        let mut applied = None;
        let preview = "working on foam/adr.md now";
        assert!(st.apply_deep_link_from_preview(preview, &mut applied));
        assert_eq!(applied.as_deref(), Some("foam/adr.md"));
        st.on_key(KeyCode::Down);
        let moved = st.selected_file().map(str::to_string);
        assert!(!st.apply_deep_link_from_preview(preview, &mut applied));
        assert_eq!(st.selected_file().map(str::to_string), moved);
    }

    #[test]
    fn list_window_keeps_selection_and_biases_high() {
        assert_eq!(foam_list_window(0, 3, 10), (0, 3));
        assert_eq!(foam_list_window(35, 40, 6), (34, 40));
        let (start, end) = foam_list_window(20, 80, 6);
        assert_eq!(end - start, 6);
        assert_eq!(20 - start, 1);
    }

    #[test]
    fn long_list_keeps_preview_visible_in_short_pane() {
        let files: Vec<(String, String)> = (0..40)
            .map(|i| (format!("f{i:02}.md"), format!("# File {i}\n\nbody {i}\n")))
            .collect();
        let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let (_t, cwd) = fixture(&refs);
        let mut st = FoamState::load(&cwd);
        st.set_size(80, 9); // the 80x24 fleet layout leaves a 9-row sidecar body
        for _ in 0..35 {
            st.on_key(KeyCode::Down);
        }
        assert_eq!(st.selected_file(), Some("foam/f35.md"));
        let preview_at = st
            .text
            .lines()
            .position(|l| l.contains("── preview ──"))
            .expect("preview marker");
        assert!(preview_at < 9, "preview too far down: line {preview_at}\n{}", st.text);
        let screen = render_fleet_foam(80, 24, &st.text).join("\n");
        assert!(screen.contains("# File 35"), "{screen}");
        assert!(screen.contains("▸     f35.md"), "{screen}");
    }
}
