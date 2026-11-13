# PLAN-296 — First-paint timing hook + dogfood Alt-P checklist

- **Status**: ready
- **PRD**: [[../prd/PRD-293-cdamux-fast-popup-open|PRD-293]]
- **GitHub**: [#296](https://github.com/chatondearu/ai-dev-kit/issues/296)
- **Date**: 2026-11-13

## Summary

Opt-in timing spans for dashboard open phases and a dogfood checklist so Alt-P
regressions are measurable.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | No telemetry crates / cloud |

## Scope

### In scope

- `CDAMUX_TIMING=1` (or similar) stderr phase logs: config, tree, status, foam, first draw
- Dogfood / architecture note for measuring popup open
- Zero cost when unset

### Out of scope

- Perf fixes themselves (#294/#295)

## Technical approach

Instant stamps around known phases in `run_dashboard`; print only when env set.

## Tasks

- [ ] Opt-in timing logs for open phases
- [ ] Document how to measure Alt-P in dogfood.md
- [ ] Guard: no logs / measurable overhead when unset

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Noisy stderr in popup | Only when env set |

## Test plan

- [ ] With env unset: no timing lines in a trivial test harness if feasible
- [ ] Docs mention the env var

## Verification

- [ ] Verify green
- [ ] PR `Closes #296`
