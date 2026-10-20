# PRD-25 — Fix kiss-audit 2026-10-20 (orch + gh-board)

- **Status**: approved
- **GitHub**: [#25](https://github.com/chatondearu/ai-dev-kit/issues/25)
- **Date**: 2026-10-20
- **Tags**: #quality #tooling #architecture

## Problem

The 2026-10-20 `/cda-kiss-audit` found concrete AFK harness defects: Status soft-match can route cards to the wrong column, CI watch waits the full timeout after a known fail, leftover worktrees block retries, `--sandbox` continues unsandboxed, `pr-fix` can `eval` verify from an untrusted PR tree, frontier fail-opens when body fetch fails, empty checks count as green, `gh-board.sh` has no tests, `.env` is always copied into durable worktrees, `orch.sh` has no flock, and worker port is dead code.

## Goals

- [ ] Findings 1–11 from that audit no longer reproduce
- [ ] Each fix is the smallest change that works; risky paths get one guarding test
- [ ] Skill/docs that contradict new sandbox behavior are updated in the same change

## Non-goals

- Auto-merge or merge babysitting
- Windows sandbox / full per-worktree runtime isolation
- Rewriting reconcile GraphQL chunking
- Deleting `archive/`
- New orch features beyond these fixes

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| AFK operator | `orch-auto` timer picks Ready cards without mis-routing Status or sticking on worktrees |
| Maintainer | `pr-fix` on an external PR cannot run arbitrary verify from the PR tree |
| Contributor | Exact Status names Ready/Done keep working; fuzzy names error clearly |

## Requirements

### Functional

- [ ] #26 — exact Status match only; stub tests for false positives
- [ ] #27 — CI fail-fast; empty checks blocked unless `ORCH_ALLOW_NO_CHECKS=1`
- [ ] #28 — worktree reuse/replace; `.env` copy only with `ORCH_COPY_ENV=1`
- [ ] #29 — sandbox hard-fail without bwrap; pr-fix verify allowlist / `ORCH_VERIFY`
- [ ] #30 — frontier hold when body fetch fails
- [ ] #31 — flock on orch modes; drop unused port calc/docs

### Non-functional

- [ ] No test touches real user config dirs
- [ ] Existing cda-dev suite stays green; new gh-board tests added
- [ ] English artifacts; Conventional Commits

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]], [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]]
- Prior audit batch: [[PRD-6-kiss-audit-fixes|PRD-6]] (different findings; shipped)

## Open questions

_(none — GO approved 2026-10-20)_

## Success metrics

- Each child’s acceptance criteria checked
- PRs CI-green and mergeable; human merges

## Implementation pointer

Per-child `plans/PLAN-<child#>-*.md` from `plans/template.md`
