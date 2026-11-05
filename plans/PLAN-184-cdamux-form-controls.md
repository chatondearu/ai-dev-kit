# PLAN-184 — cdamux form controls (tabs, fields, scrollbars)

- **Status**: ready-for-pr
- **PRD**: none (issue body is the contract; scoped from [#184](https://github.com/chatondearu/ai-dev-kit/issues/184))
- **GitHub**: [#184](https://github.com/chatondearu/ai-dev-kit/issues/184)
- **Date**: 2026-11-03

## Summary

Make the cdamux TUI look like a form-based app: button-style tabs, typed field
chrome (text / select / checkbox), and overflow scrollbars on the Fleet tree and
Settings list (reusing `widgets::draw_scrollbar`).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux\|ADR-004]] | cdamux stays in-tree, ratatui TUI; no new dependency |

## Scope

### In scope

- Tab bar as clickable button chips (hit-test: column → tab).
- Settings / spawn / composer / text-modal value chrome: input bg, `[ value ▾ ]`, `[x]`/`[ ]`.
- Fleet tree + Settings field list overflow scrollbar via `widgets::draw_scrollbar`.
- Golden snapshot regen; render tests.

### Out of scope

- Clicking / hovering fields, key rebinding, review TUI, web UI.

## Technical approach

Reuse existing `draw_scrollbar` / `draw_scrolled_list`. Settings goes through
`draw_field_list`; Fleet tree draws the bar after its `ListState` render. Form
chrome and tab buttons are separate slices.

## Tasks

- [x] **T1 scrollbars** — Fleet session tree and Settings field list show
  `widgets::draw_scrollbar` only on overflow (test: 30 items → thumb, 5 items →
  border only); golden snapshots regen, diff is scrollbar-only.
- [x] **T2 tabs** — tab bar renders as filled/unfilled `[ n Label ]` buttons;
  pure hit-test maps column → tab (gaps → none); number keys unchanged.
- [x] **T3 field chrome** — text input bg + cursor, Enum as `[ value ▾ ]`, bool as
  `[x]`/`[ ]` on Settings / spawn modal / composer / text modal; render tests.

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Snapshot churn beyond scrollbar | Review golden diff before commit; keep chrome tasks out of T1 |
| Fleet RIGHT-only border vs ALL-border lists | Draw scrollbar on full tree area (right column = border) |

## Test plan

- [x] T1: `field_list_scrollbar_only_when_overflowing`, `tree_scrollbar_only_when_overflowing`
- [x] T2: click-at-column → tab unit test
- [x] T3: render harness asserts input bg / `▾` / `[x]` / active tab fill
- [x] `cargo test` in `cdamux/`
- [x] Golden snapshots under `cdamux/crates/cdamux-cli/tests/snapshots/` reviewed

## Verification

- [x] `nix develop -c bash scripts/verify.sh`
- [x] No new crate dependency
