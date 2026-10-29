# PLAN-79 — extract ratatui widgets from settings_form

- **Status**: done
- **PRD**: [[../../prd/PRD-79-cdamux-widgets-extract|PRD-79]]
- **GitHub**: [#79](https://github.com/chatondearu/ai-dev-kit/issues/79)
- **Parent**: [#75](https://github.com/chatondearu/ai-dev-kit/issues/75) / [[../../prd/PRD-75-cdamux-keymap-settings-form|PRD-75]]
- **Date**: 2026-10-28

## Summary

Extract Toggle / Select / TextInput / FieldList from `settings_form` into
`cdamux-cli` `widgets/`, rewire Settings with unchanged behavior, optionally
share TextInput with rename. Ratatui only; no new crate.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-004-cdamux\|ADR-004]] | in-tree cdamux-cli; no install.sh enable |

## Scope

### In scope

- `widgets/` module + Settings rewire
- Optional rename modal TextInput reuse
- Tests moved/kept green

### Out of scope

- New settings fields; web UI; key rebinding

## Technical approach

1. Identify pure helpers in settings form (cycle, edit_text, modal state, field list draw/focus).
2. Lift into `widgets/{mod,select,toggle,text_input,field_list}.rs` (or fewer files if KISS).
3. Settings imports widgets; public Settings API for `tui` unchanged where possible.
4. Rename: swap local modal buffer editing to shared TextInput **if** diff stays small; else leave a one-line TODO citing #79.

## Tasks

- [x] **W1** Widgets module compiles; Select/Toggle/TextInput/FieldList cover Settings needs. *Accept:* unit tests on widget helpers without full TUI.
- [x] **W2** Settings form is a thin consumer; all #75 behaviors preserved (focus, cycle, roots, validate+save). *Accept:* existing settings_form tests still pass (moved or adapted).
- [x] **W3** Rename modal uses shared TextInput **or** deferred with comment `shortcut: rename still local, adopt when FieldList lands`. *Accept:* rename still works in tests / no regress.
- [x] **W4** Dogfood note if operator-visible; `cargo test -p cdamux-cli` green.

## Test plan

```bash
cd cdamux && nix develop ..#cdamux -c cargo test -p cdamux-cli
```

## Verification

- [ ] Tests green; PR open Closes #79; do not merge (orch)
