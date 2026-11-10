# PRD-224 — pr-fix addresses trusted review feedback, not only red CI

- **Status**: shipped
- **GitHub**: [#224](https://github.com/chatondearu/ai-dev-kit/issues/224)
- **Date**: 2026-11-10
- **Tags**: #product #orchestration

## Problem

`orch.sh --mode pr-fix` hands the agent only the PR title, body and check rollup.
A change request left on the PR (formal review, inline thread, conversation
comment) is ignored. On a repo without CI the run is a no-op that still spends an
agent call: PR #219 needed a sub-agent to apply a master review finding.

## Goals

- [x] pr-fix acts on change requests from trusted reviewers.
- [x] Untrusted PR text never reaches the agent as instructions.
- [x] The PR shows what was addressed and what was not.
- [x] No agent call when there is nothing to fix.

## Non-goals

- An independent reviewer gate after pr-fix.
- Fork PRs (already `blocked`).
- Resolving review threads automatically.

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Leaves "changes requested" on a worker PR → `orch.sh --mode pr-fix --pr N` fixes it and comments the mapping |
| Outside commenter | Posts a comment on the PR → ignored (not OWNER/MEMBER/COLLABORATOR) |
| Maintainer, green PR, no feedback | pr-fix exits `done` with no agent run |

## Requirements

### Functional

- [x] Feedback sources: reviews with state `CHANGES_REQUESTED`, unresolved review
      threads (GraphQL `reviewThreads.isResolved=false`), conversation comments
      created after the head commit; author association `OWNER|MEMBER|COLLABORATOR` only.
- [x] `orch.sh` fetches and filters; the prompt gets a `## Review feedback` section
      (author, kind, file:line when inline, body).
- [x] One summary PR comment per run: finding → commit sha, or the reason it was not addressed.
- [x] A finding conflicting with plan / ADR / product decision → `blocked` with a reason.
- [x] CI green (or allowed-no-checks) and no trusted feedback → `done` without agent.

### Non-functional

- [x] Stub-`gh` tests; never the live config.
- [x] gh lookup failure → fail closed (`blocked`), like the existing metadata guard.
- [x] `nix develop -c bash scripts/verify.sh` green.

## Related project memory

- ADRs: [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] (trusted
  authors for fast-lane briefs), [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]]

## Success metrics

- Re-running PR #219's scenario (a trusted conversation comment requesting a fix,
  no CI) produces a fix commit and a summary comment.

## Implementation pointer

`foam/archive/plans/PLAN-224-pr-fix-review-feedback.md`
