# PRD-357 — Global orchestrator across cdamux projects

- **Status**: shipped
- **GitHub**: [#357](https://github.com/chatondearu/ai-dev-kit/issues/357)
- **Date**: 2026-11-15
- **Tags**: #product #orchestration #cdamux

## Problem

The maintainer runs one orchestrator per project (ai-dev-kit, neomind, barajeuxonline,
nixos, …) and switches tmux sessions to talk to each. They want one session, started from
cdamux, that dispatches work into any discovered project. A per-project orchestrator must
remain possible.

Today a global orchestrator cannot work: `cdamux worker spawn` always uses the caller's
cwd repo (`NotGit` outside a repo), `spawn_orchestrator` has no parent, nothing caps live
workers across projects, and `cda-orchestrator` has no global routing.

## Goals

- [x] `cdamux orch new --global` starts one orchestrator outside any repo
- [x] It dispatches workers into any discovered project (`worker spawn --repo`) and runs
      `orch.sh --repo` batches there
- [x] A cap on live workers across projects protects the machine
- [x] Slice 2: it can start a project orchestrator as its child for heavy per-project work
- [x] `cdamux orch new` in a project keeps working on its own

## Non-goals

- Cross-machine / tailnet dispatch
- Msg transport changes (ADR-011 §4)
- Merging per-project orchestrators automatically into the global one

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | `cdamux orch new --global` (or Fleet `n` → global) → "fix the popup lag in cdamux and add the coolify bind in nixos" → two workers, one per repo, appear under their projects in Fleet |
| Maintainer | "neomind: big refactor" → global starts a neomind project orchestrator as its child (slice 2) and relays its summaries |
| Maintainer | 6 workers already live → `worker spawn` refuses with "6/6 live workers"; the orchestrator queues and says so |
| Maintainer | Already has a neomind orchestrator → keeps using it directly |

## Requirements

### Functional

- [x] `worker spawn --repo <path|project>` ([#363](https://github.com/chatondearu/ai-dev-kit/issues/363))
- [x] `orch new --global`, `scope=global`, one live global orchestrator, Fleet top level ([#364](https://github.com/chatondearu/ai-dev-kit/issues/364))
- [x] `[orchestrator].max_workers` cap, default 6 ([#366](https://github.com/chatondearu/ai-dev-kit/issues/366))
- [x] `cda-orchestrator` § Global; ADR-015 accepted ([#367](https://github.com/chatondearu/ai-dev-kit/issues/367))
- [x] Slice 2: nested project orchestrator with restore parents-first ([#365](https://github.com/chatondearu/ai-dev-kit/issues/365))

### Non-functional

- [x] Built on [[PRD-356-orchestrator-actually-orchestrates|PRD-356]] (boot prompt, role
      routing, hard rules): children blocked until those land
- [x] Tests on throwaway tmux servers only (`cdamux/AGENTS.md`)
- [x] Opt-in: nothing new in `install.sh` defaults

## Related project memory

- ADRs: [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]],
  [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]],
  [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]]
- Plans: [[../plans/PLAN-363-worker-spawn-repo|PLAN-363]], [[../plans/PLAN-364-orch-new-global|PLAN-364]], [[../plans/PLAN-366-global-worker-cap|PLAN-366]], [[../plans/PLAN-367-orchestrator-global-section|PLAN-367]], [[../plans/PLAN-365-nested-project-orchestrator|PLAN-365]]
