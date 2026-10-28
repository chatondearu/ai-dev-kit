//! Settings tab: focusable typed field list editing `Config` in place.
//!
//! Thin consumer of `widgets`: maps `Config` fields to Select / Toggle /
//! TextInput / FieldList.

use crate::app_tabs::{CLR_MUTED, CLR_OK, CLR_PROJECT, CLR_TEXT};
use crate::widgets::{
    cycle, draw_field_list, draw_text_modal, edit_text, move_focus, toggle_text, FieldRow,
};
use cdamux_core::{
    save_config_to, AttachMode, Config, DiscoverMode, GroupBy, ProjectRoot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::ListItem,
    Frame,
};
use std::path::{Path, PathBuf};

const ATTACH_MODES: [AttachMode; 2] = [AttachMode::Mirror, AttachMode::Exclusive];
const GROUP_BYS: [GroupBy; 5] = [
    GroupBy::Project,
    GroupBy::Cwd,
    GroupBy::Agent,
    GroupBy::GroupPath,
    GroupBy::None,
];
const DISCOVER_MODES: [DiscoverMode; 3] =
    [DiscoverMode::Flat, DiscoverMode::GitOnly, DiscoverMode::Depth];

pub const SETTINGS_FOOTER: &str =
    "j/k move · ←→/Enter change · Space toggle · a add root · d delete root · S save · 1-5 tabs · q quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    AttachMode,
    GroupBy,
    PreviewMaxLines,
    ListenTailnet,
    TailnetPort,
    IdleSeconds,
    Discover,
    MaxDepth,
    Root(usize),
    AddRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Enum,
    Bool,
    Scalar,
    Root,
    Action,
}

impl Field {
    fn kind(self) -> Kind {
        match self {
            Field::AttachMode | Field::GroupBy | Field::Discover => Kind::Enum,
            Field::ListenTailnet => Kind::Bool,
            Field::PreviewMaxLines | Field::TailnetPort | Field::IdleSeconds | Field::MaxDepth => {
                Kind::Scalar
            }
            Field::Root(_) => Kind::Root,
            Field::AddRoot => Kind::Action,
        }
    }

    fn section(self) -> &'static str {
        match self {
            Field::AttachMode | Field::GroupBy => "[sessions]",
            Field::PreviewMaxLines => "[ui]",
            Field::ListenTailnet | Field::TailnetPort | Field::IdleSeconds => "[daemon]",
            Field::Discover | Field::MaxDepth | Field::Root(_) | Field::AddRoot => "[projects]",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Field::AttachMode => "attach_mode",
            Field::GroupBy => "group_by",
            Field::PreviewMaxLines => "preview_max_lines",
            Field::ListenTailnet => "listen_tailnet",
            Field::TailnetPort => "tailnet_port",
            Field::IdleSeconds => "idle_seconds_for_needs_input",
            Field::Discover => "discover",
            Field::MaxDepth => "max_depth",
            Field::Root(_) => "root",
            Field::AddRoot => "+ add root",
        }
    }
}

pub fn fields(cfg: &Config) -> Vec<Field> {
    let mut out = vec![
        Field::AttachMode,
        Field::GroupBy,
        Field::PreviewMaxLines,
        Field::ListenTailnet,
        Field::TailnetPort,
        Field::IdleSeconds,
        Field::Discover,
        Field::MaxDepth,
    ];
    out.extend((0..cfg.projects.roots.len()).map(Field::Root));
    out.push(Field::AddRoot);
    out
}

fn value_of(cfg: &Config, field: Field) -> String {
    match field {
        Field::AttachMode => format!("{:?}", cfg.sessions.attach_mode).to_lowercase(),
        Field::GroupBy => format!("{:?}", cfg.sessions.group_by),
        Field::PreviewMaxLines => cfg.ui.preview_max_lines.to_string(),
        Field::ListenTailnet => toggle_text(cfg.daemon.listen_tailnet).into(),
        Field::TailnetPort => cfg.daemon.tailnet_port.to_string(),
        Field::IdleSeconds => cfg.daemon.idle_seconds_for_needs_input.to_string(),
        Field::Discover => format!("{:?}", cfg.projects.discover),
        Field::MaxDepth => cfg.projects.max_depth.to_string(),
        Field::Root(i) => cfg
            .projects
            .roots
            .get(i)
            .map(|r| {
                if r.label.is_empty() {
                    r.path.display().to_string()
                } else {
                    format!("{} → {}", r.label, r.path.display())
                }
            })
            .unwrap_or_default(),
        Field::AddRoot => String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Scalar { field: Field, input: String },
    /// `index = None` adds a new root; Tab switches label/path.
    Root { index: Option<usize>, label: String, path: String, on_path: bool },
    ConfirmDelete(usize),
}

#[derive(Debug, Default)]
pub struct SettingsForm {
    pub focus: usize,
    pub dirty: bool,
    pub status: String,
    pub modal: Option<Modal>,
    /// Save target; `None` (tests' default) never writes.
    pub path: Option<PathBuf>,
}

/// Checks run before any write; the first failure is returned.
pub fn validate(cfg: &Config) -> Result<(), String> {
    if cfg.daemon.tailnet_port == 0 {
        return Err("tailnet_port must be 1-65535".into());
    }
    if cfg.projects.max_depth < 1 {
        return Err("max_depth must be >= 1".into());
    }
    if cfg.projects.roots.iter().any(|r| r.path.as_os_str().is_empty()) {
        return Err("root path must not be empty".into());
    }
    Ok(())
}

impl SettingsForm {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path, ..Default::default() }
    }

    pub fn focused(&self, cfg: &Config) -> Field {
        let all = fields(cfg);
        all[self.focus.min(all.len() - 1)]
    }

    fn changed(&mut self) {
        self.dirty = true;
        self.status.clear();
    }

    /// Returns true when the key was consumed.
    pub fn handle_key(&mut self, cfg: &mut Config, key: KeyEvent) -> bool {
        if self.modal.is_some() {
            self.modal_key(cfg, key);
            return true;
        }
        if !(key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) {
            return false;
        }
        let field = self.focused(cfg);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus = move_focus(self.focus, fields(cfg).len(), 1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus = move_focus(self.focus, fields(cfg).len(), -1);
            }
            KeyCode::Left | KeyCode::Char('h') => self.step(cfg, field, -1),
            KeyCode::Right | KeyCode::Char('l') => self.step(cfg, field, 1),
            KeyCode::Char(' ') if field.kind() == Kind::Bool => self.step(cfg, field, 1),
            KeyCode::Enter => match field.kind() {
                Kind::Enum | Kind::Bool => self.step(cfg, field, 1),
                Kind::Scalar => {
                    self.modal = Some(Modal::Scalar { field, input: value_of(cfg, field) });
                }
                Kind::Root | Kind::Action => self.open_root_modal(cfg, field),
            },
            KeyCode::Char('a') => self.open_root_modal(cfg, Field::AddRoot),
            KeyCode::Char('d') => {
                if let Field::Root(i) = field {
                    self.modal = Some(Modal::ConfirmDelete(i));
                } else {
                    self.status = "d: focus a root to delete".into();
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => self.save(cfg),
            _ => return false,
        }
        true
    }

    fn step(&mut self, cfg: &mut Config, field: Field, delta: i32) {
        match field {
            Field::AttachMode => {
                cfg.sessions.attach_mode = cycle(&ATTACH_MODES, cfg.sessions.attach_mode, delta);
            }
            Field::GroupBy => {
                cfg.sessions.group_by = cycle(&GROUP_BYS, cfg.sessions.group_by, delta);
            }
            Field::Discover => {
                cfg.projects.discover = cycle(&DISCOVER_MODES, cfg.projects.discover, delta);
            }
            Field::ListenTailnet => cfg.daemon.listen_tailnet = !cfg.daemon.listen_tailnet,
            _ => return,
        }
        self.changed();
    }

    fn open_root_modal(&mut self, cfg: &Config, field: Field) {
        let (index, label, path) = match field {
            Field::Root(i) => {
                let r = &cfg.projects.roots[i];
                (Some(i), r.label.clone(), r.path.display().to_string())
            }
            _ => (None, String::new(), String::new()),
        };
        self.modal = Some(Modal::Root { index, label, path, on_path: true });
    }

    fn modal_key(&mut self, cfg: &mut Config, key: KeyEvent) {
        let Some(modal) = self.modal.as_mut() else { return };
        if key.code == KeyCode::Esc {
            self.modal = None;
            return;
        }
        match modal {
            Modal::ConfirmDelete(i) => {
                let i = *i;
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                    cfg.projects.roots.remove(i);
                    self.focus = self.focus.min(fields(cfg).len() - 1);
                    self.changed();
                }
                self.modal = None;
            }
            Modal::Scalar { field, input } => {
                if key.code == KeyCode::Enter {
                    let (field, input) = (*field, input.trim().to_string());
                    match set_scalar(cfg, field, &input) {
                        Ok(()) => {
                            self.modal = None;
                            self.changed();
                        }
                        Err(e) => self.status = e,
                    }
                } else {
                    edit_text(input, key, |_| true);
                }
            }
            Modal::Root { index, label, path, on_path } => match key.code {
                KeyCode::Tab | KeyCode::BackTab => *on_path = !*on_path,
                KeyCode::Enter => {
                    let path = path.trim();
                    if path.is_empty() {
                        self.status = "root path must not be empty".into();
                        return;
                    }
                    if !Path::new(path).is_absolute() {
                        self.status = "root path must be absolute".into();
                        return;
                    }
                    let root = ProjectRoot { path: PathBuf::from(path), label: label.trim().into() };
                    match *index {
                        Some(i) => cfg.projects.roots[i] = root,
                        None => {
                            cfg.projects.roots.push(root);
                            // Focus sits just above the trailing AddRoot row.
                            self.focus = fields(cfg).len() - 2;
                        }
                    }
                    self.modal = None;
                    self.changed();
                }
                _ => edit_text(if *on_path { path } else { label }, key, |_| true),
            },
        }
    }

    pub fn save(&mut self, cfg: &Config) {
        if let Err(e) = validate(cfg) {
            self.status = format!("not saved: {e}");
            return;
        }
        let Some(path) = &self.path else {
            self.status = "no config path".into();
            return;
        };
        match save_config_to(cfg, path) {
            Ok(()) => {
                self.dirty = false;
                self.status = "saved config".into();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
}

fn set_scalar(cfg: &mut Config, field: Field, input: &str) -> Result<(), String> {
    let bad = |_| format!("{}: not a valid number", field.label());
    match field {
        Field::PreviewMaxLines => cfg.ui.preview_max_lines = input.parse().map_err(bad)?,
        Field::TailnetPort => cfg.daemon.tailnet_port = input.parse().map_err(bad)?,
        Field::IdleSeconds => cfg.daemon.idle_seconds_for_needs_input = input.parse().map_err(bad)?,
        Field::MaxDepth => cfg.projects.max_depth = input.parse().map_err(bad)?,
        _ => {}
    }
    Ok(())
}

pub fn draw_settings(f: &mut Frame, area: Rect, cfg: &Config, form: &SettingsForm, config_path: &str) {
    let dirty_tag = if form.dirty { " *" } else { "" };
    let header = ListItem::new(Line::from(vec![
        Span::styled("config: ", Style::default().fg(CLR_MUTED)),
        Span::styled(format!("{config_path}{dirty_tag}"), Style::default().fg(CLR_TEXT)),
    ]));
    let all = fields(cfg);
    let focus = form.focus.min(all.len() - 1);
    let rows = all
        .into_iter()
        .map(|field| FieldRow {
            section: field.section(),
            label: field.label(),
            value: value_of(cfg, field),
            value_color: match field {
                Field::ListenTailnet if cfg.daemon.listen_tailnet => CLR_OK,
                Field::ListenTailnet | Field::AddRoot => CLR_MUTED,
                _ => CLR_PROJECT,
            },
            action: field.kind() == Kind::Action,
        })
        .collect();
    let status = if form.status.is_empty() { "S: save to disk" } else { form.status.as_str() };
    draw_field_list(f, area, "settings", header, rows, focus, status);

    if let Some(modal) = &form.modal {
        let (title, body) = modal_text(modal, cfg);
        draw_text_modal(f, area, title, body);
    }
}

fn modal_text(modal: &Modal, cfg: &Config) -> (String, String) {
    match modal {
        Modal::Scalar { field, input } => (
            format!(" {} ", field.label()),
            format!("value: {input}█\nEnter save · Esc cancel"),
        ),
        Modal::Root { index, label, path, on_path } => {
            let (lc, pc) = if *on_path { ("", "█") } else { ("█", "") };
            (
                if index.is_some() { " Edit root ".into() } else { " Add root ".into() },
                format!(
                    "label: {label}{lc}\npath:  {path}{pc}\nTab switch field · Enter save · Esc cancel"
                ),
            )
        }
        Modal::ConfirmDelete(i) => (
            " Delete root ".into(),
            format!("delete {}?\ny/Enter confirm · Esc cancel", value_of(cfg, Field::Root(*i))),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdamux_core::load_config_from;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typ(form: &mut SettingsForm, cfg: &mut Config, text: &str) {
        for c in text.chars() {
            form.handle_key(cfg, key(KeyCode::Char(c)));
        }
    }

    fn focus_on(form: &mut SettingsForm, cfg: &Config, field: Field) {
        form.focus = fields(cfg).iter().position(|f| *f == field).unwrap();
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("cdamux-settings-{}-{name}", std::process::id()))
            .join("config.toml")
    }

    #[test]
    fn focus_moves_and_clamps() {
        let mut cfg = Config::default();
        let mut form = SettingsForm::default();
        form.handle_key(&mut cfg, key(KeyCode::Char('k')));
        assert_eq!(form.focus, 0);
        form.handle_key(&mut cfg, key(KeyCode::Char('j')));
        assert_eq!(form.focused(&cfg), Field::GroupBy);
        for _ in 0..50 {
            form.handle_key(&mut cfg, key(KeyCode::Down));
        }
        assert_eq!(form.focused(&cfg), Field::AddRoot);
    }

    #[test]
    fn enum_cycles_and_bool_toggles() {
        let mut cfg = Config::default();
        let mut form = SettingsForm::default();
        focus_on(&mut form, &cfg, Field::GroupBy);
        form.handle_key(&mut cfg, key(KeyCode::Left));
        assert_eq!(cfg.sessions.group_by, GroupBy::None);
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert_eq!(cfg.sessions.group_by, GroupBy::Project);
        assert!(form.dirty);

        focus_on(&mut form, &cfg, Field::ListenTailnet);
        let before = cfg.daemon.listen_tailnet;
        form.handle_key(&mut cfg, key(KeyCode::Char(' ')));
        assert_eq!(cfg.daemon.listen_tailnet, !before);
    }

    #[test]
    fn scalar_modal_sets_value_and_rejects_garbage() {
        let mut cfg = Config::default();
        let mut form = SettingsForm::default();
        focus_on(&mut form, &cfg, Field::MaxDepth);
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        // Modal captures digits and `q` instead of tab switch / quit.
        for _ in 0..5 {
            form.handle_key(&mut cfg, key(KeyCode::Backspace));
        }
        typ(&mut form, &mut cfg, "x");
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert!(form.modal.is_some());
        assert!(form.status.contains("not a valid number"));
        form.handle_key(&mut cfg, key(KeyCode::Backspace));
        typ(&mut form, &mut cfg, "4");
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert!(form.modal.is_none());
        assert_eq!(cfg.projects.max_depth, 4);
    }

    #[test]
    fn save_validates_then_writes() {
        let path = temp_path("validate");
        let _ = std::fs::remove_file(&path);
        let mut cfg = Config::default();
        let mut form = SettingsForm { dirty: true, ..SettingsForm::new(Some(path.clone())) };

        cfg.projects.max_depth = 0;
        form.save(&cfg);
        assert!(form.status.contains("max_depth"));
        assert!(form.dirty && !path.exists());

        cfg.projects.max_depth = 2;
        cfg.daemon.tailnet_port = 0;
        form.save(&cfg);
        assert!(form.status.contains("tailnet_port") && !path.exists());

        cfg.daemon.tailnet_port = 8787;
        form.save(&cfg);
        assert_eq!(form.status, "saved config");
        assert!(!form.dirty && path.exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn roots_add_edit_delete_round_trip() {
        let path = temp_path("roots");
        let mut cfg = Config::default();
        cfg.projects.roots.clear();
        let mut form = SettingsForm::new(Some(path.clone()));

        // Add: empty path rejected, then a valid one.
        form.handle_key(&mut cfg, key(KeyCode::Char('a')));
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert!(form.status.contains("must not be empty"));
        assert!(cfg.projects.roots.is_empty());
        typ(&mut form, &mut cfg, "/src/one");
        form.handle_key(&mut cfg, key(KeyCode::Tab));
        typ(&mut form, &mut cfg, "one");
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert_eq!(form.focused(&cfg), Field::Root(0));
        form.handle_key(&mut cfg, key(KeyCode::Char('a')));
        typ(&mut form, &mut cfg, "/src/two");
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert_eq!(cfg.projects.roots.len(), 2);

        // Edit root 0 path.
        focus_on(&mut form, &cfg, Field::Root(0));
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        typ(&mut form, &mut cfg, "-x");
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert_eq!(cfg.projects.roots[0].path, PathBuf::from("/src/one-x"));
        assert_eq!(cfg.projects.roots[0].label, "one");

        // Delete root 1 (Esc cancels, y confirms).
        focus_on(&mut form, &cfg, Field::Root(1));
        form.handle_key(&mut cfg, key(KeyCode::Char('d')));
        form.handle_key(&mut cfg, key(KeyCode::Esc));
        assert_eq!(cfg.projects.roots.len(), 2);
        form.handle_key(&mut cfg, key(KeyCode::Char('d')));
        form.handle_key(&mut cfg, key(KeyCode::Char('y')));
        assert_eq!(cfg.projects.roots.len(), 1);

        form.handle_key(&mut cfg, key(KeyCode::Char('S')));
        assert_eq!(form.status, "saved config");
        let loaded = load_config_from(&path).unwrap();
        let roots: Vec<_> =
            loaded.projects.roots.iter().map(|r| (r.label.as_str(), r.path.clone())).collect();
        assert_eq!(roots, vec![("one", PathBuf::from("/src/one-x"))]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn draws_modal_on_tiny_terminal() {
        let cfg = Config::default();
        let form = SettingsForm { modal: Some(Modal::ConfirmDelete(0)), ..Default::default() };
        let mut term =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(12, 4)).unwrap();
        term.draw(|f| draw_settings(f, f.area(), &cfg, &form, "/x")).unwrap();
    }

    #[test]
    fn relative_root_rejected() {
        let mut cfg = Config::default();
        let n = cfg.projects.roots.len();
        let mut form = SettingsForm::default();
        form.handle_key(&mut cfg, key(KeyCode::Char('a')));
        typ(&mut form, &mut cfg, "src");
        form.handle_key(&mut cfg, key(KeyCode::Enter));
        assert!(form.status.contains("absolute"));
        assert_eq!(cfg.projects.roots.len(), n);
    }
}
