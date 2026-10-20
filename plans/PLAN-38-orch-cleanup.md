# PLAN-38 — orch worktree cleanup

- **Status**: in-progress
- **PRD**: [[../prd/PRD-33-cda-aitools|PRD-33]]
- **GitHub**: [#38](https://github.com/chatondearu/ai-dev-kit/issues/38)
- **Date**: 2026-10-20

## Summary

Safe cleanup for leftover `.orch/worktrees` with dry-run.

## Tasks

- [ ] orch-cleanup.sh or orch.sh --mode cleanup
- [ ] Dry-run; only remove registered/stale worktrees
- [ ] Test
- [ ] Doc pointer

## Verification

- [ ] test_worktree or new test green
