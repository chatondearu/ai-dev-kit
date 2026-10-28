# PRD-75 — cdamux keymap discoverability + editable Settings (roots)

- **Status**: shipped
- **GitHub**: [#75](https://github.com/chatondearu/ai-dev-kit/issues/75)
- **Parent epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Date**: 2026-10-28
- **Tags**: #product #tooling #ux

## Problem

Session actions **rename** (`m`), **fork** (`f`), and **kill** (`x`) already work in the
Fleet TUI, but the always-visible footer omits them — only the `?` overlay lists them.
Operators think the shortcuts are missing.

The **Settings** tab (PRD-41 / #63) is display + ad-hoc key cycles (`a`/`g`/`+/-`/`t`),
not a form: no focusable fields, no text entry, and **project roots** (a primary
config pain) cannot be added/edited/deleted in the UI.

## Goals

- [x] **Keymap registry**: one table drives `map_key_event`, Fleet footer, and `?`
  overlay; Fleet footer includes `m` / `f` / `x`.
- [x] **Settings form**: focusable typed field list (enum cycle, bool toggle,
  number/string via text modal).
- [x] **Roots**: add / edit / delete `projects.roots` from Settings; persist with `S`.
- [x] **Modules** shaped for a fast follow-up extracting `widgets/` (approach 2)
  — follow-up [#79](https://github.com/chatondearu/ai-dev-kit/issues/79).
- [x] Update `cdamux/docs/dogfood.md` key table and Settings checklist.

## Non-goals

- User-configurable key rebinding
- Editing agents command map or notes editor path in TUI
- Tailnet static web UI settings parity
- Shipping the full widget toolkit in this issue (tracked as follow-up after merge)
- Enabling cdamux from `install.sh` (ADR-004 / ADR-003)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Operator | On Fleet, glance footer → see `m` rename → press `m` → rename modal |
| Operator | Press `?` → same bindings as footer, grouped |
| Maintainer | Tab `5` Settings → edit roots (add `/hdd/dev`, fix label) → `S` → rediscover |
| Maintainer | Toggle `listen_tailnet`, bump `preview_max_lines`, cycle `attach_mode` without opening TOML |

## Requirements

### Functional

#### Keymap / help

- [ ] Introduce `keymap` module: entries `{ key, action, tabs, help_line }`.
- [ ] Fleet footer summary includes session manage keys (`m`/`f`/`x`) plus existing nav.
- [ ] `?` overlay renders from the same registry (grouped: nav / session / tabs).
- [ ] Key behavior for rename/fork/kill/attach/etc. unchanged unless documented.

#### Settings form

- [ ] Navigable list (`j`/`k` or arrows); focused row highlighted.
- [ ] Field kinds: enum (←→ or Enter cycle), bool (Space/Enter toggle), scalar
      (Enter → text modal, same UX family as rename).
- [ ] Editable in v1: `attach_mode`, `group_by`, `preview_max_lines`, `listen_tailnet`,
      `tailnet_port`, `idle_seconds_for_needs_input`, `discover`, `max_depth`, roots.
- [ ] Dirty marker `*` + `S` save to existing config path; status line on error/success.
- [ ] Validation: non-empty root path; `max_depth >= 1`; port fits `u16`; reject save
      on validation failure with status message.

#### Roots

- [ ] List each root as `label → path` (label may be empty → show path only).
- [ ] `a` (or “+ add root” row): modal for label + path.
- [ ] Enter on root: edit modal; `d`: delete (confirm Enter / cancel Esc).
- [ ] Saved roots round-trip through existing TOML serialize path.

### Non-functional

- [ ] Unit tests: keymap maps critical Fleet keys; footer/help strings include `m`/`f`/`x`.
- [ ] Unit/integration: settings cycle/toggle/edit; root add/delete; save to temp TOML.
- [ ] No new UI crate dependency; ratatui only.
- [ ] English comments; Conventional Commits on the implementation PR.

## Design notes (approved)

**Approach**: form-as-field-list now; extract `Toggle` / `Select` / `TextInput` /
`FieldList` into `widgets/` in a **follow-up** issue immediately after.

**Layout (ASCII)**:

```
┌─ Settings ──────────────────────────────────────────┐
│ config: ~/.config/cdamux/config.toml *              │
│ [sessions] attach_mode / group_by                   │
│ [ui] preview_max_lines                              │
│ [daemon] listen_tailnet / tailnet_port / idle_…     │
│ [projects] discover / max_depth / roots list + add  │
│ S save · Esc blur · 1-5 tabs                        │
└─────────────────────────────────────────────────────┘
```

**Code shape (v1)**: `keymap.rs`, `settings_form.rs` under `cdamux-cli`; reuse rename
modal pattern for text entry — do not invent a second input stack.

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]]
- Parent PRD: [[PRD-41-cdamux-mission-control-tui|PRD-41]] mission control
- Prior Settings tab: issue [#63](https://github.com/chatondearu/ai-dev-kit/issues/63)
- Epic: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)

## Open questions

_(none — resolved in design: approach 1 now / 2 next; roots in v1)_

## Success metrics

- Dogfood: operator finds rename without opening source or guessing
- Roots editable end-to-end without leaving the TUI
- Follow-up widgets issue can cite concrete extract points in `settings_form.rs`

## Implementation pointer

`plans/PLAN-75-cdamux-keymap-settings-form.md` from `plans/template.md`
