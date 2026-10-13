# PLAN-9 — README fixes

- **Status**: in-progress
- **PRD**: [[../prd/PRD-6-kiss-audit-fixes|PRD-6]]
- **GitHub**: [#9](https://github.com/chatondearu/ai-dev-kit/issues/9)
- **Date**: 2026-10-13

## Summary

Remove the root README's duplicated skill table and stale per-category list in favor of the single `skills/README.md` index, and tell Nix users how to get kiss hooks.

## Constraints (from ADRs)

None recorded. Never cut validation, error handling or the non-destructive merge guarantees (`kiss`).

## Tasks

- [ ] Replace the root 'Included skills' table and the stale layout comments with a link to `skills/README.md`
- [ ] In the Nix install section, state that the Nix path links skills only and kiss hooks need one `./install.sh` run

## Test plan

Doc-only change: verify every relative link in the root README resolves and no other instruction changed.

## Verification

- [ ] `for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh; do bash "$t"; done`
- [ ] `bash skills/quality/kiss/hooks/test.sh`
