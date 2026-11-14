# PLAN-331 — feat(cdamux): sidebar hover and click-to-activate

- **Status**: ready
- **PRD**: [[../prd/PRD-329-cdamux-sidebar-hover-click-activate|PRD-329-cdamux-sidebar-hover-click-activate]]
- **GitHub**: [#331](https://github.com/chatondearu/ai-dev-kit/issues/331)
- **Date**: 2026-11-14

## Summary

Enable mouse in `cdamux sidebar`: hover highlight on tree rows, left-click on a
live session runs the Enter activate path. Keyboard switch debounce stays.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Ratatui TUI; switch-client / follow for the invoking client |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Session hierarchy unchanged; attach only attachable session rows |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Mouse capture in sidebar `--in-pane`
- Hover style ≠ selection
- Click → Enter activate for live sessions
- Footer hint; tests / snapshot

### Out of scope

- Popup Fleet mouse
- Hover-to-switch
- Dormant restore from sidebar

## Technical approach

Reuse Fleet tree hit-testing patterns from the dashboard mouse path. Sidebar
today calls `DisableMouseCapture` so tmux focuses the pane; switch to enable
capture and handle `Moved` / `Down(Left)`. Activate via existing
`follow_session` + `focus_main_pane` (Attach arm). Hover is paint-only — do not
set `pending` switch debounce from hover.

Graph plan: `plans/graph/PLAN-331-sidebar-mouse.json`.

## Tasks

- [ ] Enable mouse capture and track hovered row from mouse move (clear when
      leaving the tree); draw hover style distinct from selection
- [ ] Left-click: live session → Attach path; project → fold/select; dormant →
      current Enter message; update footer
- [ ] Add snapshot/unit coverage; keep throwaway-tmux e2e green if touched

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Mouse capture steals tmux click-to-focus | Intentional; process owns hit-test like Fleet dashboard |
| Hover thrash / switch | Hover must not call `follow_session` |

## Test plan

- Unit/snapshot: hover style and/or click maps to activate for a session row
- Existing `sidebar_tmux` e2e still passes on throwaway `-L` server
- Manual: hover visible; click focuses main pane; j/k still switches

## Verification

- [ ] `cargo test` in `cdamux/` (host PATH; nix only if toolchain missing)
- [ ] No popup Fleet mouse regressions in shared draw helpers
