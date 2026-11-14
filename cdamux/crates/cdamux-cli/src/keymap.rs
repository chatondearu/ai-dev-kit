//! Fleet keymap registry: one table drives key mapping, footer, and `?` help.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardAction {
    None,
    Quit,
    Attach,
    Spawn,
    SelectNext,
    SelectPrev,
    Collapse,
    Expand,
    FocusComposer,
    CycleSideTab,
    StartReview,
    ToggleWaitingFilter,
    FoldAll,
    CycleGroupBy,
    /// Full-screen drawn orch graph of the selected row's repo.
    GraphView,
    ToggleHelp,
    KillSession,
    ForkSession,
    RenameSession,
    /// Open GitHub issue for `feat/<n>-*` / worktree (via `gh issue view --web`).
    OpenIssue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Navigate,
    Session,
    View,
    App,
}

impl Group {
    const ALL: [Group; 4] = [Group::Navigate, Group::Session, Group::View, Group::App];

    fn title(self) -> &'static str {
        match self {
            Group::Navigate => "Navigate",
            Group::Session => "Sessions",
            Group::View => "View",
            Group::App => "App",
        }
    }
}

pub struct Binding {
    /// Keys matched by `map_key_event`; empty = handled elsewhere (help-only row).
    pub codes: &'static [KeyCode],
    pub ctrl: bool,
    pub label: &'static str,
    pub action: DashboardAction,
    pub help: &'static str,
    /// Short footer label; `None` keeps the binding in `?` only.
    pub footer: Option<&'static str>,
    pub group: Group,
}

const fn b(
    codes: &'static [KeyCode],
    label: &'static str,
    action: DashboardAction,
    help: &'static str,
    footer: Option<&'static str>,
    group: Group,
) -> Binding {
    Binding { codes, ctrl: false, label, action, help, footer, group }
}

use DashboardAction as A;
use Group as G;
use KeyCode as K;

pub const FLEET_KEYMAP: &[Binding] = &[
    b(&[K::Down, K::Char('j')], "j ↓", A::SelectNext, "next row", Some("j/k nav"), G::Navigate),
    b(&[K::Up, K::Char('k')], "k ↑", A::SelectPrev, "previous row", None, G::Navigate),
    b(&[K::Left, K::Char('h')], "h ←", A::Collapse, "collapse row", None, G::Navigate),
    b(&[K::Right, K::Char('l')], "l →", A::Expand, "expand row", None, G::Navigate),
    b(&[K::Char('F')], "F", A::FoldAll, "fold/unfold all groups", None, G::Navigate),
    b(&[], "click", A::None, "select row", None, G::Navigate),
    b(&[K::Enter], "Enter", A::Attach, "attach / restore", Some("Enter attach"), G::Session),
    b(&[K::Char('n')], "n", A::Spawn, "new session", Some("n new"), G::Session),
    b(&[K::Char('m')], "m", A::RenameSession, "rename session", Some("m rename"), G::Session),
    b(&[K::Char('f')], "f", A::ForkSession, "fork session", Some("f fork"), G::Session),
    b(&[K::Char('x')], "x", A::KillSession, "kill / forget (x x)", Some("x kill"), G::Session),
    b(&[K::Char('o')], "o", A::OpenIssue, "open GitHub issue", Some("o issue"), G::Session),
    b(&[K::Char(' ')], "Space", A::FocusComposer, "send text", Some("Space send"), G::Session),
    Binding { codes: &[K::Char('r')], ctrl: true, label: "Ctrl+R", action: A::StartReview, help: "hunk review TUI", footer: None, group: G::Session },
    b(&[K::Tab], "Tab", A::CycleSideTab, "next sidecar tab", None, G::View),
    b(&[], "PgUp PgDn [ ]", A::None, "scroll preview", None, G::View),
    b(&[], "t (Foam tab)", A::None, "focus Foam tree", None, G::View),
    b(&[], "m (Foam focus)", A::None, "Foam raw / rendered", None, G::View),
    b(&[K::Char('w')], "w", A::ToggleWaitingFilter, "waiting filter (◆)", None, G::View),
    b(&[K::Char('g')], "g", A::GraphView, "Graph view (Tab: run)", Some("g graph"), G::View),
    b(&[K::Char('G')], "G", A::CycleGroupBy, "cycle group_by", None, G::View),
    b(&[], "1-5", A::None, "app tabs", Some("1-5 tabs"), G::App),
    b(&[K::Char('?')], "?", A::ToggleHelp, "this help", Some("? help"), G::App),
    b(&[K::Char('q')], "q", A::Quit, "quit", Some("q quit"), G::App),
    Binding { codes: &[K::Char('c')], ctrl: true, label: "Ctrl+C", action: A::Quit, help: "quit", footer: None, group: G::App },
];

pub fn map_key_event(key: KeyEvent) -> DashboardAction {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let hit = FLEET_KEYMAP
        .iter()
        .find(|bind| bind.ctrl == ctrl && bind.codes.contains(&key.code));
    hit.map_or(A::None, |bind| bind.action)
}

pub fn fleet_footer() -> String {
    FLEET_KEYMAP
        .iter()
        .filter_map(|bind| bind.footer)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Help rows for `groups`; the `?` overlay splits the groups into two columns when short.
pub fn fleet_help_groups(groups: &[Group]) -> String {
    let mut out = String::new();
    for group in groups {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(group.title());
        out.push('\n');
        for bind in FLEET_KEYMAP.iter().filter(|bind| bind.group == *group) {
            out.push_str(&format!("  {:<14} {}\n", bind.label, bind.help));
        }
    }
    out
}

pub const HELP_LEFT: [Group; 2] = [Group::Navigate, Group::Session];
pub const HELP_RIGHT: [Group; 2] = [Group::View, Group::App];

pub fn fleet_help_text() -> String {
    fleet_help_groups(&Group::ALL)
}

/// Keep the footer on one row: drop items (right to left) before the last two
/// (`? help · q quit`), so the way out stays visible on narrow terminals.
pub fn fit_footer(footer: &str, width: usize) -> String {
    let mut items: Vec<&str> = footer.split(" · ").collect();
    let len = |items: &[&str]| items.join(" · ").chars().count();
    while len(&items) > width && items.len() > 2 {
        items.remove(items.len() - 3);
    }
    items.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn session_keys_map() {
        assert_eq!(map_key_event(key(K::Char('m'))), A::RenameSession);
        assert_eq!(map_key_event(key(K::Char('f'))), A::ForkSession);
        assert_eq!(map_key_event(key(K::Char('x'))), A::KillSession);
        assert_eq!(map_key_event(key(K::Char('k'))), A::SelectPrev);
        assert_eq!(map_key_event(key(K::Down)), A::SelectNext);
        assert_eq!(map_key_event(key(K::Esc)), A::None);
        assert_eq!(map_key_event(key(K::Char('g'))), A::GraphView);
        assert_eq!(map_key_event(key(K::Char('G'))), A::CycleGroupBy);
        assert_eq!(map_key_event(KeyEvent::new(K::Char('c'), KeyModifiers::CONTROL)), A::Quit);
        assert_eq!(map_key_event(KeyEvent::new(K::Char('m'), KeyModifiers::CONTROL)), A::None);
    }

    #[test]
    fn footer_lists_session_keys() {
        let footer = fleet_footer();
        for part in ["g graph", "m rename", "f fork", "x kill", "o issue", "Enter attach", "? help"] {
            assert!(footer.contains(part), "{footer}");
        }
    }

    #[test]
    fn h_collapses_l_expands() {
        assert_eq!(map_key_event(key(K::Char('h'))), A::Collapse);
        assert_eq!(map_key_event(key(K::Left)), A::Collapse);
        assert_eq!(map_key_event(key(K::Char('l'))), A::Expand);
        assert_eq!(map_key_event(key(K::Right)), A::Expand);
    }

    #[test]
    fn footer_fits_width_and_keeps_quit() {
        let footer = fleet_footer();
        assert_eq!(fit_footer(&footer, 500), footer);
        let short = fit_footer(&footer, 40);
        assert!(short.chars().count() <= 40, "{short}");
        assert!(short.starts_with("j/k nav") && short.ends_with("? help · q quit"), "{short}");
        assert_eq!(fit_footer("a · b · c", 1), "b · c");
    }

    #[test]
    fn help_is_grouped_and_complete() {
        let help = fleet_help_text();
        assert!(help.contains("Sessions\n"));
        for line in ["rename session", "fork session", "kill / forget (x x)", "attach / restore"] {
            assert!(help.contains(line), "{help}");
        }
    }
}
