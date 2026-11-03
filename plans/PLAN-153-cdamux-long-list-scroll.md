# PLAN-153 — cdamux long lists follow cursor and scroll

- **Status**: verify
- **PRD**: none (quickwin, intake 2026-11-03; issue body is the contract)
- **GitHub**: [#153](https://github.com/chatondearu/ai-dev-kit/issues/153)
- **Date**: 2026-11-03

## Summary

Long lists in the cdamux TUI (spawn modal Running/Discovered, Sessions tab,
Projects tab) lose the cursor past the bottom of the viewport and ignore the
mouse wheel. Make them follow the cursor and scroll.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux\|ADR-004]] | cdamux stays in-tree, ratatui TUI; no new dependency |

## Scope

### In scope

- `cdamux/crates/cdamux-cli/src/spawn_modal.rs` — Running / Discovered lists
  rendered as a `List` with a `ListState` kept in `ModalState` (offset persists
  between frames); mouse wheel moves `pick_index` by 3.
- `cdamux/crates/cdamux-cli/src/app_tabs.rs` — Sessions: fix the `start + i`
  double offset (or switch to `List` + persisted `ListState`); Projects: subtract
  the 2 border rows from `visible`.
- `cdamux/crates/cdamux-cli/src/tui.rs` — Sessions key handler stops pinning
  `scroll = selected`; mouse wheel routed to the spawn modal and the Sessions /
  Projects tabs. Keep this diff minimal (#139 edits the same file).

### Out of scope

- Fleet tree (`fleet_draw.rs`) — #139.
- Review TUI list, PgUp/PgDn bindings.

## Technical approach

Prefer ratatui's own scrolling: `List` + `ListState` whose offset survives
between frames (store the state, render with `render_stateful_widget`), instead
of hand-rolled `skip/take` windows. Only keep a manual window where a cursor is
not wanted (Projects tab).

## Tasks

- [x] Spawn modal: `List` + persisted `ListState` for Running / Discovered
- [x] Spawn modal: mouse wheel moves the selection (±3, clamped)
- [x] Sessions tab: highlight correct while scrolled; cursor moves freely, list scrolls at edges
- [x] Sessions tab: mouse wheel moves the selection
- [x] Projects tab: border rows subtracted; last project reachable; wheel scrolls
- [x] Test: 30 items, 10-row area, select 25 → row 25 rendered and highlighted (modal + Sessions)

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Merge conflict with #139 in `tui.rs` | Minimal `tui.rs` diff; rebase on whichever merges second |
| `draw_modal` needs `&mut ModalState` for the persisted offset | One call site in `fleet_draw.rs`; adjust it |

## Test plan

- [x] `cargo test` in `cdamux/` (TestBackend render test)
- [ ] Manual: 30+ tmux sessions on an isolated `tmux -L` server, scroll each list

## Verification

- [ ] `bash scripts/verify.sh` — 1 unrelated failure: `test_verify_gate.sh`
  "reviewer prompt has WORKTREE" (orch.sh gates, #130); every other suite green
- [x] Update foam / ADR if a new decision was made (none expected)
