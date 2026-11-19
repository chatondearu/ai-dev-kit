# PRD-403 — Foam-first kanban, worker meta auto-merge, global resume index

- **Status**: approved
- **GitHub**: [#403](https://github.com/chatondearu/ai-dev-kit/issues/403)
- **Date**: 2026-11-19
- **Tags**: #product #orchestration #foam #cdamux

## Problem

Kanban status lives on GitHub Projects. Agents spend tokens on the Projects
API, `orch-auto` cannot list Ready without `gh-board`, and a global
orchestrator (ADR-015) has no one-shot view of in-progress / Ready / blockers
across repos. Foam is already the agent memory graph; execution status is a
second store. Meta PRs (`prd/`, `plans/`, ADRs) still wait on a maintainer
merge click.

## Goals

- [ ] Project status lives in `foam/board/` (one file per card); GitHub
      Issues are optional; GitHub Projects is not the source of truth
- [ ] `orch.sh` / `orch-auto` / `cda-kanban` list Ready from foam
- [ ] Workers self-merge PRs whose diff is only the meta allowlist, after
      verify + ADR-006 review; fail closed otherwise
- [ ] `$CDAMUX_HOME/board-index.json` is a cache of per-repo foam boards for
      the global orch; git remains the sync
- [ ] Kit dogfood, then every non-Beamy cdamux project (checklist; no GH
      issues on those repos)

## Non-goals

- Reopening or expanding epic [#265](https://github.com/chatondearu/ai-dev-kit/issues/265)
  (Foam browser / Fleet sidecar). Link it where board files are visible in
  Fleet; do not change that UI here.
- GitHub ruleset, merge queue, or automerge app
- Auto-merge of `doc/`, README, skills, cdamux crates, or product code
- Treating the `$CDAMUX_HOME` index as a second source of truth
- Touching Beamy repos or `beamysoftware/*`
- Opening GitHub issues on rollout targets
- Cross-machine / tailnet dispatch
- A second kanban (new columns, a different status vocabulary)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Kit worker | Picks a `status: ready` card from `foam/board/`; if `github_issue` is set, uses `Closes #N`; otherwise foam-only |
| AFK orch-auto | Lists Ready from foam; no Projects API in the default path |
| Global orch | `cat $CDAMUX_HOME/board-index.json` (after refresh) sees Ready / in-progress / blockers / open PRs across non-Beamy projects |
| Worker on a plans-only PR | After verify + review, `gh pr merge` itself; maintainer is not asked |
| Worker on a skill + plan PR | Fail closed; reports for maintainer merge |
| Refresh on a repo without foam | Index records the skip; process continues |

## Requirements

### Functional

- [ ] #404 — ADR-016 + card contract + documentation-layout
- [ ] #405 — skills + orch list-ready from foam; `cda-orchestrator` reads the global index; `gh issue` only when a card opts in
- [ ] #406 — worker self-merge on the allowlist; fail closed outside it
- [ ] #407 — `$CDAMUX_HOME/board-index.json` + refresh; skip Beamy; missing foam does not crash
- [ ] #408 — rollout checklist of non-Beamy repos; kit first; one-shot map then stop writing Projects

### Non-functional

- [ ] One card = one file (merge-conflict constraint)
- [ ] Index rebuild is idempotent; tests use throwaway `$CDAMUX_HOME`
- [ ] ADR-006 review gate still runs before any self-merge
- [ ] English artifacts; Conventional Commits
- [ ] Verify: `bash scripts/verify.sh` on shipping PRs

## Children

| Issue | Scope |
| ----- | ----- |
| [#404](https://github.com/chatondearu/ai-dev-kit/issues/404) | ADR-016 + `foam/board` contract |
| [#405](https://github.com/chatondearu/ai-dev-kit/issues/405) | Skills + orch list-ready from foam |
| [#406](https://github.com/chatondearu/ai-dev-kit/issues/406) | Worker meta auto-merge (`Blocked by: #405`) |
| [#407](https://github.com/chatondearu/ai-dev-kit/issues/407) | cdamux global index (`Blocked by: #404`) |
| [#408](https://github.com/chatondearu/ai-dev-kit/issues/408) | Rollout checklist (`Blocked by: #405`) |

## Related project memory

- ADRs: [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]],
  [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]],
  [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]],
  [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]],
  [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]]
- Foam browser epic: [#265](https://github.com/chatondearu/ai-dev-kit/issues/265)
  / [[PRD-265-cdamux-foam-browser|PRD-265]] — overlap only (browse vs board SoT)
- Board contract: [[../foam/board/README|foam/board/README]]
- Layout: [[../foam/product/documentation-layout|documentation-layout]]
- Plans: [[../plans/PLAN-404-adr-016-foam-first-kanban|PLAN-404]],
  [[../plans/PLAN-405-skills-foam-board|PLAN-405]],
  [[../plans/PLAN-406-worker-meta-automerge|PLAN-406]],
  [[../plans/PLAN-407-cdamux-board-index|PLAN-407]],
  [[../plans/PLAN-408-foam-board-rollout|PLAN-408]]

## Open questions

_(none — owner GO 2026-11-19; card format is a constraint, not a bikeshed)_

## Success metrics

- Ready pickup in the kit works with Projects API disabled
- Global orch resume from the index without per-repo Projects calls
- Meta PRs merge without a maintainer ask; mixed diffs do not

## Implementation pointer

Per-child `plans/PLAN-<child#>-*.md`. First implementation Ready: **#405**
after #404 lands. #407 may run in parallel with #405 once #404 is closed.
