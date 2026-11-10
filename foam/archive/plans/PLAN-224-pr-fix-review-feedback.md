# PLAN-224 — pr-fix addresses trusted review feedback

- **Status**: done
- **PRD**: [[../../prd/PRD-224-pr-fix-review-feedback|PRD-224]]
- **GitHub**: [#224](https://github.com/chatondearu/ai-dev-kit/issues/224)
- **Date**: 2026-11-10

## Summary

`orch.sh` collects trusted change requests on the PR and injects them into the
pr-fix prompt; the worker fixes them, posts one summary comment, and pr-fix skips
the agent entirely when there is nothing to fix.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | Only `OWNER`/`MEMBER`/`COLLABORATOR` text counts; reuse the existing helper in `orch.sh` (~line 122) |
| [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]] | gh failures fail closed |

## Scope

### In scope

- `skills/core/cda-dev/scripts/orch.sh` pr-fix path (after the fork/default-branch
  guard, before rendering the prompt):
  - `orch_pr_review_feedback <num>` (new, in orch.sh or `lib/`): prints markdown items from
    1. `gh pr view --json reviews` → state `CHANGES_REQUESTED`, trusted author;
    2. `gh api graphql` `reviewThreads(first:100){isResolved,path,line,comments(first:20){author,authorAssociation,body}}` → unresolved threads whose first comment is trusted;
    3. `gh pr view --json comments,commits` → trusted comments with `createdAt` after the last commit's `committedDate`.
    Lookup failure → `blocked` (fail closed).
  - Skip: feedback empty **and** current checks green (or none with allow-no-checks) →
    `done: PR N nothing to fix (checks green, no trusted feedback)`, no agent.
  - Pass feedback to the prompt (`REVIEW_FEEDBACK` file path or appended section, follow
    how ready-pickup appends retry sections).
- `skills/core/cda-dev/prompts/pr-fix.md`: `## Review feedback` handling — address each
  item, commit, push; then one `gh pr comment` mapping item → sha or reason; never
  resolve threads; conflict with plan/ADR/product → BLOCKED.
- Docs: `doc/orchestration.md` pr-fix section, `skills/core/cda-dev/SKILL.md` modes row.

### Out of scope

- Reviewer gate after pr-fix, fork PRs, thread resolution.

## Tasks

- [x] feedback collector + trust filter
- [x] nothing-to-fix skip
- [x] prompt section + summary comment instructions
- [x] tests (stub gh) + docs

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Prompt injection via PR text | trust filter in code; feedback framed as data in the prompt |
| Re-addressing old comments every run | comments only after the head commit; threads only unresolved |

## Test plan

- [x] New `tests/test_pr_fix_feedback.sh` (or extend `test_pr_fix_gate.sh`): stub gh returns
      reviews/threads/comments with mixed associations and dates → only trusted, unresolved,
      post-head items in the prompt; empty + green → no agent call; gh failure → blocked
- [x] `nix develop -c bash scripts/verify.sh`
