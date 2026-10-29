# PLAN-28 — Worktree reuse on retry and opt-in env copy

- **Status**: done
- **PRD**: [[../../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- **GitHub**: [#28](https://github.com/chatondearu/ai-dev-kit/issues/28)
- **Date**: 2026-10-20

## Summary

Reuse or replace leftover worktrees on AFK retry; copy `.env` only when `ORCH_COPY_ENV=1`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | orch-auto must retry Ready cards without human cleanup |

## Scope

### In scope

- `orch_worktree_add` / bootstrap behavior
- `test_worktree.sh`
- Document `ORCH_COPY_ENV` in conventions

### Out of scope

- Changing default force-clean after success

## Tasks

- [ ] If path exists and is expected branch worktree → reuse (rc 0)
- [ ] Else remove stale and recreate
- [ ] `.env` copy only when ORCH_COPY_ENV=1
- [ ] Tests for reuse + env opt-in

## Test plan

- [ ] `test_worktree.sh` green

## Verification

- [ ] `bash skills/core/cda-dev/scripts/tests/test_worktree.sh`
