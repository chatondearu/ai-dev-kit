# PLAN-75 — cdamux keymap + Settings form (roots)

- **Status**: done
- **PRD**: [[../prd/PRD-75-cdamux-keymap-settings-form|PRD-75]]
- **GitHub**: [#75](https://github.com/chatondearu/ai-dev-kit/issues/75)
- **Parent epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Date**: 2026-10-28

## Summary

Make Fleet session shortcuts (`m`/`f`/`x`) discoverable via a single keymap
registry (footer + `?`), and replace the Settings tab’s ad-hoc key cycles with a
focusable typed field list that can add/edit/delete `projects.roots` and save
with existing `S` / `save_config`. Shape modules for a follow-up `widgets/`
extract (not in this plan).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux\|ADR-004]] | cdamux stays in-tree CLI+daemon; no `install.sh` enable |
| [[../foam/decisions/ADR-003-cda-aitools\|ADR-003]] | aitools / orch worktrees not auto-bridged; roots are explicit config |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Keymap registry → `map_key_event`, Fleet footer, `?` overlay
- Settings field list: enums, bools, scalars (text modal), roots CRUD
- Persist via existing config save path; validation before save
- Dogfood key table + Settings checklist update
- Unit tests at keymap + settings-form seams

### Out of scope

- Key rebinding; agents/notes path editors; web UI; shipping `widgets/` crate
  (open follow-up issue after merge)

## Technical approach

Tracer bullets in `cdamux-cli` (ratatui only):

1. **Keymap** — pure table `{ key, modifiers?, action, tabs, help_line, group }`;
   mapping and help string builders have no Frame dependency.
2. **Settings form** — `FieldKind` + focus index + optional text/root modal state;
   draw/handle isolated from Fleet loop; mutate `Config` in place; `try_save` /
   `save_config` unchanged contract.
3. **Text modal** — reuse rename-modal UX (Esc cancel, Enter confirm); root edit
   uses label + path (two-step or two fields in one modal — pick one and stick).
4. **Wiring** — Settings tab keys go through form handler; Fleet help strings
   from keymap; remove duplicated hard-coded `FLEET_HELP` / thin footer that omit
   `m`/`f`/`x`.

## Tasks

### Slice A — Keymap (discoverability)

- [x] **A1** Keymap table covers all current Fleet dashboard actions with stable
  `DashboardAction` (or equivalent) mapping; `m`→rename, `f`→fork, `x`→kill
  unchanged. *Accept:* unit test maps those three keys.
- [x] **A2** Fleet footer string is derived from the registry and includes
  `m`/`f`/`x` (and existing nav). *Accept:* assertion on footer content.
- [x] **A3** `?` overlay content is derived from the same registry (grouped).
  *Accept:* help text contains rename/fork/kill lines; no second hand-maintained
  full help string.

### Slice B — Settings field list (no roots yet)

- [x] **B1** Focusable field rows for: `attach_mode`, `group_by`,
  `preview_max_lines`, `listen_tailnet`, `tailnet_port`,
  `idle_seconds_for_needs_input`, `discover`, `max_depth`. *Accept:* j/k moves
  focus; focused row visually distinct.
- [x] **B2** Enum cycle (←→/Enter), bool toggle (Space/Enter), scalar Enter→text
  modal. *Accept:* unit tests flip/cycle/set values on a `Config` without TUI.
- [x] **B3** Dirty `*` + `S` save; validation rejects bad port / `max_depth < 1`
  with status message and no write. *Accept:* save-to-temp success + validation
  failure cases.

### Slice C — Roots CRUD

- [x] **C1** Roots listed under projects section; add (`a` or add-row), edit
  (Enter), delete (`d` + confirm). *Accept:* unit tests mutate
  `cfg.projects.roots`.
- [x] **C2** Round-trip: edit roots → save → reload TOML equals expected roots.
  *Accept:* temp-dir integration test via `save_config` / load.
- [x] **C3** Empty path rejected on confirm/save. *Accept:* status error, no
  empty root persisted.

### Slice D — Docs + follow-up hook

- [x] **D1** Update `cdamux/docs/dogfood.md` mission-control key table (`m`/`f`/`x`)
  and Settings edit checklist.
- [x] **D2** After implementation PR merges: open follow-up issue for `widgets/`
  extract citing `settings_form` seams — [#79](https://github.com/chatondearu/ai-dev-kit/issues/79).

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Footer too long / wraps badly | Short labels; keep full detail in `?` only |
| Modal stacks (spawn + rename + settings) | One overlay at a time; Settings modal exclusive while open |
| Roots path UX ambiguous | Require absolute path string; no directory picker in v1 |
| Regress existing Settings hotkeys | Preserve `S` save; document that letter cycles move to field focus |

## Test plan

**Seams under test**

1. Keymap: key event → action; registry → footer/help strings  
2. Settings form: key → `Config` mutation + dirty flag  
3. Config I/O: validate → `save_config` → load equality (temp path)

```bash
cd cdamux && cargo test -p cdamux-cli && cargo test -p cdamux-core && cargo build -p cdamux-cli
```

Manual dogfood: Fleet footer shows `m`; Settings add a root → `S` → restart sees root.

## Verification

- [x] Lint / test / build as above
- [x] Plan tasks ticked; PRD → `shipped` on merge
- [x] Follow-up widgets issue filed (D2 → #79)
- [x] Foam MoC PRD-75 → shipped
