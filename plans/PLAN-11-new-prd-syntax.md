# PLAN-11 — new-prd.sh syntax error

- **Status**: in-progress
- **PRD**: [[../prd/PRD-6-kiss-audit-fixes|PRD-6]]
- **GitHub**: [#11](https://github.com/chatondearu/ai-dev-kit/issues/11)
- **Date**: 2026-10-13

## Summary

`new-prd.sh` has a stray quote in its issue-body extraction, so it never parses. Fix it and guard the whole class of bug.

## Constraints (from ADRs)

None recorded. Never cut validation, error handling or the non-destructive merge guarantees (`kiss`).

## Tasks

- [ ] Fix the issue-body extraction so the empty-body fallback is a valid jq expression
- [ ] Add a cheap syntax guard (`bash -n` over every kit shell script) to the existing test runner

## Test plan

Seam: the script's CLI.
- [ ] `bash -n` passes on all `skills/**/*.sh`
- [ ] `new-prd.sh --dry-run 6` prints the target PRD path
- [ ] the syntax guard fails when a script is broken (verified once by hand)

## Verification

- [ ] `for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh; do bash "$t"; done`
- [ ] `bash skills/quality/kiss/hooks/test.sh`
