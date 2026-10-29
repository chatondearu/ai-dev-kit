# PLAN-31 — Flock orch modes and drop unused worker port

- **Status**: done
- **PRD**: [[../../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- **GitHub**: [#31](https://github.com/chatondearu/ai-dev-kit/issues/31)
- **Date**: 2026-10-20

## Summary

Serialize `ready-pickup` / `pr-fix` with flock (like orch-auto); remove dead port calculation and docs.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | One auto run at a time; orch itself should not race with it |

## Scope

### In scope

- flock on shared `.orch/orch.lock` (or similar) for ready-pickup/pr-fix
- Remove port calc from orch.sh; update conventions.md
- Smoke still passes

### Out of scope

- Changing orch-auto lock path semantics beyond coexistence

## Tasks

- [ ] Acquire flock at start of ready-pickup and pr-fix; if held, exit 0 with message (match orch-auto)
- [ ] Remove unused port variables/echoes and ORCH_BASE_PORT docs
- [ ] Smoke dry-run still works

## Test plan

- [ ] smoke_orch_dry.sh + syntax tests

## Verification

- [ ] Relevant cda-dev tests pass
