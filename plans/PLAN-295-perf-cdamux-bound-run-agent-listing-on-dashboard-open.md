# PLAN-295 — Bound run/agent listing on dashboard open

- **Status**: ready
- **PRD**: [[../prd/PRD-293-cdamux-fast-popup-open|PRD-293]]
- **GitHub**: [#295](https://github.com/chatondearu/ai-dev-kit/issues/295)
- **Date**: 2026-11-13

## Summary

Make `list_runs_for_repo` / StatusCache refresh cheap when `$CDAMUX_HOME` holds
hundreds of run/agent files — avoid full-dir mtime sort + unbounded parse on
first paint.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | State under `$CDAMUX_HOME`; no inventing a new store |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local FS |

## Scope

### In scope

- Cap / early-exit / smarter ordering for run listing used at dashboard open
- Similar care for agent status loads if they dominate open
- Tests with many fixture JSON files

### Out of scope

- Foam lazy load (#294)
- Changing retention GC policy except documenting interaction

## Technical approach

Profile `list_runs_for_repo`: today sorts all entries by mtime then reads until
limit. Prefer: limit candidates before full parse, or maintain a cheap index,
or stop after `limit` live/recent matches without sorting the entire directory
when unnecessary. Keep Fleet live/recent UX for the selected repo.

## Tasks

- [ ] Bound or short-circuit run listing on open path
- [ ] Ensure per-repo live/recent results remain correct
- [ ] Tests with large synthetic runs dir
- [ ] Note dogfood: GC still recommended for hundreds of stale files

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Missing a live run | Prefer live detection before aggressive caps |

## Test plan

- [ ] Fixture dir with N≫limit files: open path stays O(limit)-ish
- [ ] Live run still appears for matching repo

## Verification

- [ ] Verify green
- [ ] PR `Closes #295`
