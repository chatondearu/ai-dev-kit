# PLAN-27 — CI watch fail-fast and empty-checks gate

- **Status**: done
- **PRD**: [[../../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- **GitHub**: [#27](https://github.com/chatondearu/ai-dev-kit/issues/27)
- **Date**: 2026-10-20

## Summary

Fail CI watch immediately on `fail`; treat empty checks as blocked unless `ORCH_ALLOW_NO_CHECKS=1`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | Autonomy stops at CI-green + mergeable; empty checks are not green |

## Scope

### In scope

- `orch_pr_checks_watch` / classify empty-array behavior
- `test_verify_ci.sh` updates
- Document `ORCH_ALLOW_NO_CHECKS` in conventions

### Out of scope

- Changing mergeable UNKNOWN handling

## Tasks

- [ ] On status fail, return red immediately
- [ ] `[]` → not green unless ORCH_ALLOW_NO_CHECKS=1
- [ ] Update tests that expected empty→green

## Test plan

- [ ] Stub gh fail returns within one sleep cycle
- [ ] Empty checks blocked by default; allowed with env

## Verification

- [ ] `bash skills/core/cda-dev/scripts/tests/test_verify_ci.sh`
