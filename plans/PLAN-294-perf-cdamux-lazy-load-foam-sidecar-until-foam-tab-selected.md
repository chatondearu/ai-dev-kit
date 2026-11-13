# PLAN-294 — Lazy-load Foam sidecar until Foam tab selected

- **Status**: ready
- **PRD**: [[../prd/PRD-293-cdamux-fast-popup-open|PRD-293]]
- **GitHub**: [#294](https://github.com/chatondearu/ai-dev-kit/issues/294)
- **Date**: 2026-11-13

## Summary

Stop walking/rendering foam on every session focus when the visible sidecar is
not Foam; load only when the Foam tab is selected (ADR-012 preserved).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]] | Foam browser remains correct when used |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local FS only |

## Scope

### In scope

- Gate `foam_sidecar_for_cwd` / preview / deep-link on Foam tab visibility
- Tests/snapshots for Preview-first vs Foam-tab load
- Document deep-link trigger rule in PR

### Out of scope

- Run listing perf (#295)
- Changing default tmux bind (#297)

## Technical approach

In Fleet TUI loop: only refresh foam state when `side_tab == Foam` (or on
transition into Foam). Keep preview/tmux capture on Preview interval as today.

## Tasks

- [ ] Defer foam list/preview until Foam tab active
- [ ] Deep-link: run when entering Foam and/or while Foam visible — one rule
- [ ] Tests: Preview open does not require foam fixtures; Foam tab still works
- [ ] Update snapshots if needed

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Deep-link surprises | Document rule; test preview→Foam |

## Test plan

- [ ] Unit/TUI: session change on Preview does not call foam load (spy or cheap assert)
- [ ] Foam tab still lists + previews

## Verification

- [ ] `bash scripts/verify.sh` or nix fallback
- [ ] PR `Closes #294`
