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
    ToggleExpand,
    FocusComposer,
    CycleSideTab,
    StartReview,
    ToggleWaitingFilter,
    FoldAll,
    CycleGroupBy,
    ToggleHelp,
    KillSession,
    ForkSession,
    RenameSession,
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
    b(&[], "click", A::None, "select row in tree", None, G::Navigate),
    b(&[K::Left, K::Right, K::Char('h'), K::Char('l')], "h/l ←→", A::ToggleExpand, "expand/collapse project", None, G::Navigate),
    b(&[K::Char('F')], "F", A::FoldAll, "fold/unfold all groups", None, G::Navigate),
    b(&[K::Enter], "Enter", A::Attach, "attach session (or review tab)", Some("Enter attach"), G::Session),
    b(&[K::Char('n')], "n", A::Spawn, "new session modal", Some("n new"), G::Session),
    b(&[K::Char('m')], "m", A::RenameSession, "rename session", Some("m rename"), G::Session),
    b(&[K::Char('f')], "f", A::ForkSession, "fork session (same cwd, new name)", Some("f fork"), G::Session),
    b(&[K::Char('x')], "x", A::KillSession, "kill session", Some("x kill"), G::Session),
    b(&[K::Char(' ')], "Space", A::FocusComposer, "composer (Preview tab)", Some("Space send"), G::Session),
    Binding { codes: &[K::Char('r')], ctrl: true, label: "Ctrl+R", action: A::StartReview, help: "hunk review TUI", footer: None, group: G::Session },
    b(&[K::Tab], "Tab", A::CycleSideTab, "Preview / Notes / Review / Orch graph", None, G::View),
    b(&[], "[/] PgUp/PgDn", A::None, "scroll preview", None, G::View),
    b(&[K::Char('w')], "w", A::ToggleWaitingFilter, "waiting-for-input filter (◆)", None, G::View),
    b(&[K::Char('g')], "g", A::CycleGroupBy, "cycle group_by (project/cwd/agent/…)", None, G::View),
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

pub fn fleet_help_text() -> String {
    let mut out = String::new();
    for group in Group::ALL {
        out.push_str(group.title());
        out.push('\n');
        for bind in FLEET_KEYMAP.iter().filter(|bind| bind.group == group) {
            out.push_str(&format!("  {:<14} {}\n", bind.label, bind.help));
        }
    }
    out.push_str("\nNote: agent-manager `g` is group tree; here `g` cycles list grouping.");
    out
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
        assert_eq!(map_key_event(KeyEvent::new(K::Char('c'), KeyModifiers::CONTROL)), A::Quit);
        assert_eq!(map_key_event(KeyEvent::new(K::Char('m'), KeyModifiers::CONTROL)), A::None);
    }

    #[test]
    fn footer_lists_session_keys() {
        let footer = fleet_footer();
        for part in ["m rename", "f fork", "x kill", "Enter attach", "? help"] {
            assert!(footer.contains(part), "{footer}");
        }
    }

    #[test]
    fn help_is_grouped_and_complete() {
        let help = fleet_help_text();
        assert!(help.contains("Sessions\n"));
        for line in ["rename session", "fork session", "kill session"] {
            assert!(help.contains(line), "{help}");
        }
    }
}
