# PLAN-8 — kiss hooks fixes

- **Status**: in-progress
- **PRD**: [[../prd/PRD-6-kiss-audit-fixes|PRD-6]]
- **GitHub**: [#8](https://github.com/chatondearu/ai-dev-kit/issues/8)
- **Date**: 2026-10-13

## Summary

The mode tracker reacts only to the `/kiss` command itself, 'stop kiss' persists for the project, and the repo map shows the repo's shape.

## Constraints (from ADRs)

None recorded. Never cut validation, error handling or the non-destructive merge guarantees (`kiss`).

## Tasks

- [ ] Command matching accepts `/kiss` alone or followed by a known level, nothing else (`/kiss-audit`, `/kiss-anything` are no-ops)
- [ ] Turning kiss off records an explicit off state for the project instead of clearing it, so the next session start stays silent; `/kiss` turns it back on
- [ ] The repo map lists unique directories up to depth 2 within the existing char budget

## Test plan

Seam: the shared hook lib's public functions (command parsing, mode read/write, instruction build, map build) — same seam as the existing lib smoke test.
- [ ] `/kiss-audit` and `/kiss-audit src/` → no-op; `/kiss ultra` / `/kiss` / `/kiss off` unchanged
- [ ] off → simulated session start emits nothing; `/kiss` → active again
- [ ] map of a fixture tree shows directories, not files, and stays within budget

## Verification

- [ ] `for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh; do bash "$t"; done`
- [ ] `bash skills/quality/kiss/hooks/test.sh`
