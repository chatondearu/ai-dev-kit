# PLAN-336 — Fleet UI dismiss finished or orphan under-session rows

- **Status**: ready
- **PRD**: [[../prd/PRD-334-cdamux-fleet-hygiene-navigator|PRD-334]]
- **GitHub**: [#336](https://github.com/chatondearu/ai-dev-kit/issues/336)
- **Blocked by**: #335
- **Date**: 2026-11-14

## Summary

Add a Fleet/navigator keybound dismiss for finished or orphan under-session runs/workers, wired to the shared finalize/sweep helpers; show `d` in footer/help.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | One write path; no parallel store |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Never dismiss-wipe resumable dormant |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | UI reads tmux + CDAMUX_HOME |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `d` (PRD keybinds) on selected finished/orphan under-session row
- Shared finalize/sweep call; confirmation when action spans multiple entities
- Footer / `?` help entry

### Out of scope

- Navigator host (#337) — but keymap must be reusable there
- Changing retention days / systemd timer

## Technical approach

Extend Fleet keymap (`DashboardAction`) with Dismiss; call existing finalize/orphan helpers for the selected run/agent id. Disable or no-op on live working hosts and on ADR-009 dormant.

## Tasks

- [ ] **Keymap**: register `d` → dismiss; footer + `?` list it (acceptance: help shows `d`)
- [ ] **Action**: selected finished/orphan row → shared finalize/sweep; protect dormant; confirm if multi-entity (acceptance: state files terminal / archived; UI row gone on refresh)
- [ ] **Tests**: state path + keymap mapping
- [ ] **Verify**: cdamux tests green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Accidental dismiss of live work | Only eligible terminal/orphan rows; confirm multi |
| Dup logic vs CLI | Reuse `finalize_*` / `sweep_orphans` helpers only |

## Test plan

Seams: (1) eligibility, (2) keymap, (3) dormant keep.

- [ ] Unit tests with temp CDAMUX_HOME
- [ ] Optional throwaway tmux if UI path needs it

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR only if a new decision was made
