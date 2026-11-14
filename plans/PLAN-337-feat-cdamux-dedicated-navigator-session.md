# PLAN-337 — dedicated navigator session replacing join-pane sidebar

- **Status**: done
- **PRD**: [[../prd/PRD-334-cdamux-fleet-hygiene-navigator|PRD-334]]
- **GitHub**: [#337](https://github.com/chatondearu/ai-dev-kit/issues/337)
- **Date**: 2026-11-14

## Summary

Replace `cdamux sidebar` join-pane with a dedicated navigator tmux session (`cdamux-nav` default) that never injects panes into agent sessions; ship Alt-N example bind, show keybinds in UI footer/`?`, document in dogfood; deprecate join-pane sidebar.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Example binds only — do not install into user tmux.conf from kit |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux is the host DB |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Session model unchanged; navigator is another session |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Same Fleet tree / state readers |

Do not violate accepted ADRs without superseding them. If #321 join-pane is retired, add a short ADR or amend ADR-004/docs — prefer a small ADR under `foam/decisions/` when the contract changes.

## Scope

### In scope

- Dedicated navigator session/window (not join-pane into agents)
- Live Fleet tree navigation + activate/focus; quit without killing agents
- Keybinds per PRD (Alt-N example, in-UI footer/`?`)
- Deprecate `cdamux sidebar` (message → navigator command) or remove in same PR
- Dogfood: replace `M-s` sidebar example with `M-n` navigator

### Out of scope

- Changing Alt-P popup behavior
- Implementing dismiss semantics (#336) beyond sharing keymap table if already present
- Multi-client layout beyond “navigator is its own session”

## Technical approach

New subcommand or mode (e.g. `cdamux nav`) that ensures session `cdamux-nav` and runs the Fleet tree full-window (reuse draw/keymap). Switching sessions uses `switch-client` only — never `join-pane` into the target. Sidebar launcher becomes a thin redirect or removal.

## Tasks

- [x] **Host**: ensure dedicated navigator session; open Fleet UI there without splitting agent panes (acceptance: agent session pane count unchanged when opening nav)
- [x] **Navigation**: j/k, Enter activate, q quit nav; `?` help; footer shows primary binds including future `d` if keymap shared (acceptance: dogfood checklist)
- [x] **tmux example**: document `Alt-N` → switch to/from navigator; keep Alt-P (acceptance: dogfood section)
- [x] **Deprecate sidebar**: `cdamux sidebar` prints redirect or is removed; docs updated (acceptance: no join-pane dogfood as primary)
- [x] **ADR/docs**: record supersession of #321 join-pane UX if removed
- [x] **Tests**: throwaway tmux — open nav, switch session, pane count on agent session stable
- [x] **Verify**: cdamux tests green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Operators still bound to M-s | Dogfood + sidebar deprecation message |
| Nav session name clash | Unique default `cdamux-nav`; document override if config exists |

## Test plan

Seams: (1) ensure nav session, (2) switch-client without join-pane, (3) keymap footer/`?`.

- [x] Throwaway tmux server (`TMUX_TMPDIR` + `-L`)
- [x] Snapshot/footer test if render harness applies

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR if join-pane contract is superseded
