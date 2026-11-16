# PLAN-374 — fix(cda-dev): orch.sh handles PRs merged mid-run and workers that end without a PR

- **Status**: ready
- **PRD**: none (fix; issue body is the spec)
- **GitHub**: [#374](https://github.com/chatondearu/ai-dev-kit/issues/374)
- **Date**: 2026-11-16

## Summary

Two ready-pickup gaps: (1) a PR the maintainer merges mid-run is not noticed — the
worker keeps iterating and the PR lookup only sees open PRs; (2) a worker can pass
implement + verify yet leave no commit/PR.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR-006 | Review gate unchanged; a merged PR is the maintainer's decision → `done` |
| ADR-002 | Lock and run layout unchanged |

## Scope

### In scope

- `skills/core/cda-dev/scripts/orch.sh` (PR lookup ~line 334, review loop ~line 358)
- `skills/core/cda-dev/scripts/lib/graph.sh` (before each implement/fix iteration)
- One small helper, e.g. `orch_pr_state <branch>` → `OPEN|MERGED|CLOSED|` (in `lib/ci.sh`)
- Tests: `skills/core/cda-dev/scripts/tests/test_pr_merged_midrun.sh` (stub `gh` on PATH)

### Out of scope

- Auto-rebase of conflicting PRs (#282), run-end vocabulary (#277), `orch-auto.sh`

## Technical approach

1. `orch_pr_state`: `gh pr list --head "$branch" --state all --json number,state` →
   first PR's state (empty if none, fail-open on `gh` error).
2. Merged check before every graph iteration, before the PR lookup and before each review
   round: `MERGED` → `orch_progress_set "$id" done "merged by maintainer"`, `rc=0`, return.
3. PR lookup accepts `--state all` and keeps the first OPEN or MERGED PR.
4. No PR after a green loop: one bounded agent step with a fixed prompt
   ("commit all changes, push `$branch`, open PR with `Closes #$id`"), then re-lookup;
   still none → `blocked` as today. No loop.

## Tasks

- [x] `orch_pr_state` helper (+ stub-gh unit test)
- [x] Merged short-circuit in graph loop, PR lookup, review rounds
- [x] PR lookup with merged PRs
- [x] One "open the PR" step when the PR is missing
- [x] Tests for both cases; `run-tests.sh` picks them up
- [x] `bash scripts/verify.sh`; PR `Closes #374`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Extra `gh` calls per iteration | One call per iteration; fail-open |
| Closed-unmerged PR | Treat like no PR (unchanged behaviour) |

## Test plan

- [x] `bash skills/core/cda-dev/scripts/tests/run-tests.sh` (via verify)
- [x] `bash scripts/verify.sh`
