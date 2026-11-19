# PLAN-406 — feat(cda-dev): worker self-merge of foam/prd/plans/decisions PRs

- **Status**: in-review
- **PRD**: [[../prd/PRD-403-foam-first-kanban|PRD-403]]
- **GitHub**: [#406](https://github.com/chatondearu/ai-dev-kit/issues/406)
- **Date**: 2026-11-19

## Summary

When the PR diff is only the ADR-016 allowlist, the worker merges it after
verify + ADR-006 review. No maintainer ask. Any other path in the diff →
fail closed (report, do not merge).

Blocked by: #405 (same `cda-dev` surface; stack, do not fan-out).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]] | Allowlist + fail closed; worker `gh pr merge`; no ruleset/app |
| [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]] | Independent review still required before merge; reviewer/verifier never merge |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | "No auto-merge" narrowed to non-allowlist diffs; poller still does not merge product PRs |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Path allowlist check on the PR diff (merge-base…HEAD)
- `gh pr merge` from orch/worker after verify + `REVIEW: approved` + checks
  green-or-none-per-repo-policy
- Worker/orchestrator contract: meta PRs do not ask the maintainer to merge
- Fail closed + a clear report when the diff is mixed
- Tests for allowlist vs reject

### Out of scope

- GitHub rulesets, merge queues, automerge apps
- Expanding the allowlist to `doc/` / README / skills (owner later)
- Changing review-gate policy
- Merging from the planner / orchestrator session

## Technical approach

One predicate: every changed path matches the allowlist (prefix rules in
ADR-016). Call it once after the review gate, before `done`. True →
`gh pr merge` (merge commit or squash: match repo default; do not
force-push). False → current behaviour (PR open, maintainer merges).
Never merge on red checks. `orch: allow-no-checks` still applies as today.

Amend cda-dev hard rule 4 and worker preamble to: merge only when the
predicate is true.

## Tasks

- [x] Allowlist predicate matches ADR-016 prefixes; a diff with only `prd/`+`plans/`+`foam/decisions/`+`foam/board/` is true; a diff that also has `doc/` or `skills/` is false
- [x] After verify + independent review + checks policy, true → worker/orch runs `gh pr merge` and reports merged; false → does not merge and says so
- [x] Worker and cda-dev docs state the allowlist in the positive (what to merge), plus fail-closed as the guardrail
- [x] No GitHub ruleset, app, or workflow is added
- [x] Tests cover both branches; `bash scripts/verify.sh`; PR `Closes #406`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Rename/chmod outside allowlist sneaks in | Predicate uses every path `git diff --name-only`; any miss → false |
| Merge with failing checks | Reuse existing checks gate; do not add a second |
| Orchestrator skill still says "never merge" | #405 may land first; this issue updates the merge sentence only |

## Test plan

Seams: the allowlist function (list of paths → bool); orch post-review
branch (merge vs report). Do not hit live GitHub merge in unit tests.

- [x] Fixture path lists: allow / reject
- [x] Harness stub: merge command invoked only on allow
- [x] `bash scripts/verify.sh`

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR if a new decision was made (none; ADR-016 already records the allowlist)
