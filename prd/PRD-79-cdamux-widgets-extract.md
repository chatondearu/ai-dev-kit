# PRD-79 — cdamux extract ratatui widgets from settings_form

- **Status**: approved
- **GitHub**: [#79](https://github.com/chatondearu/ai-dev-kit/issues/79)
- **Parent epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Follows**: [#75](https://github.com/chatondearu/ai-dev-kit/issues/75) / [[PRD-75-cdamux-keymap-settings-form|PRD-75]]
- **Date**: 2026-10-28
- **Tags**: #tooling #ux #architecture

## Problem

PRD-75 shipped a typed Settings form with reusable seams (`Field`/`Kind`,
`Modal`, `edit_text`, field list draw) still living inside `settings_form.rs`.
Rename (and later spawn) duplicate text-modal UX. Approach 2 from the PRD-75
design was to extract `Toggle` / `Select` / `TextInput` / `FieldList` next.

## Goals

- [ ] Extract a small `widgets/` module under `cdamux-cli` (ratatui only).
- [ ] Rewire Settings as a consumer; behavior unchanged (keys, validation, roots).
- [ ] Optionally reuse `TextInput` for the rename modal.
- [ ] Keep unit tests on the same seams; no new UI crate.

## Non-goals

- Key rebinding; agents/notes path editors; web UI forms
- New Settings fields
- Enabling cdamux from `install.sh` (ADR-004)

## Requirements

### Functional

- [ ] Widgets cover: enum cycle (Select), bool (Toggle), text modal (TextInput),
  focusable list (FieldList) enough for Settings.
- [ ] Settings dogfood checklist still passes (same keys as post-#75).
- [ ] Rename modal uses shared text input **or** documented deferral if risky.

### Non-functional

- [ ] `cargo test -p cdamux-cli` green; no new dependency.
- [ ] Extract points cited in #79 body remain the guide.

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]]
- Prior art: [[PRD-75-cdamux-keymap-settings-form|PRD-75]] (keymap + Settings form)

## Implementation pointer

`foam/archive/plans/PLAN-79-cdamux-widgets-extract.md`
