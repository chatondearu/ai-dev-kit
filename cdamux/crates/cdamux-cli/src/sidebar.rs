//! `cdamux sidebar` (#321) — deprecated in favour of `cdamux nav` (#337).
//!
//! Join-pane into agent sessions is no longer the primary navigator. The clap
//! surface stays so old `M-s` binds print a redirect instead of failing on argv.

use cdamux_core::SidebarSide;
use std::process::ExitCode;

pub(crate) fn cmd_sidebar(
    _side: SidebarSide,
    _width: u16,
    _client: Option<String>,
    _in_pane: bool,
) -> ExitCode {
    eprintln!("cdamux sidebar is deprecated; use `cdamux nav` (dedicated session, no join-pane).");
    eprintln!(
        "Example bind: bind-key -n M-n run-shell \"cdamux nav --client '#{{client_name}}'\""
    );
    ExitCode::from(2)
}
