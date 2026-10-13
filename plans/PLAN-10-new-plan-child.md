# PLAN-10 — new-plan.sh: child plans from the epic PRD

- **Status**: done
- **PRD**: — (trivial-scope waiver: single-script fix, see cda-plan "Plan-without-PRD only for trivial scope")
- **GitHub**: [#10](https://github.com/chatondearu/ai-dev-kit/issues/10)
- **Date**: 2026-10-13

## Summary

`cda-dev` batches put one PRD on the epic and one plan per child, but `new-plan.sh <child#>` only looks for `prd/PRD-<child#>-*.md` and dies. When the issue has no PRD of its own, fall back to the PRD of the issue named in its `## Parent` section.

## Constraints (from ADRs)

None recorded. KISS: no new option — the `## Parent` convention already exists in cda-dev child bodies.

## Tasks

- [x] When no PRD exists for the issue, read the issue body's `## Parent` section (`#N`) and use the PRD of #N; the plan's PRD link points to that epic PRD
- [x] Keep the error when neither the issue nor its parent has a PRD, and make it name both numbers it tried
- [x] Single-issue behavior (issue has its own PRD) unchanged

## Test plan

Seam: the script's CLI against a temp git repo (`origin` = this GitHub repo, `prd/` + `plans/` scaffold), using real closed issues: #7 (child, `## Parent #6`) and #6 (has `PRD-6`).
- [x] `new-plan.sh 7` in a repo holding only `prd/PRD-6-*.md` → creates `plans/PLAN-7-*.md` whose PRD link is `PRD-6-…`
- [x] `new-plan.sh 6` with `PRD-6` present → unchanged behavior
- [x] an issue with no PRD and no parent PRD → clear error, exit non-zero
- [x] `bash -n` (covered by `test_syntax.sh`)

## Verification

- [x] `for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh; do bash "$t"; done`
- [x] `bash skills/quality/kiss/hooks/test.sh`
