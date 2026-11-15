# PRD-356 — Orchestrator sessions actually orchestrate

- **Status**: approved
- **GitHub**: [#356](https://github.com/chatondearu/ai-dev-kit/issues/356)
- **Date**: 2026-11-15
- **Tags**: #product #orchestration #cdamux

## Problem

cdamux orchestrator sessions ([[PRD-239-orchestrator-session|PRD-239]]) code the
maintainer's requests themselves instead of dispatching workers. Kiss-audit 2026-11-15:

- The boot prompt is pasted right after spawn and lost by `cursor-agent`: none of the
  Cursor orchestrator chats on the maintainer's machine received it. `a-x3lo6` made 201
  file edits and 0 `cdamux worker spawn`.
- Session-start routing (kiss hooks) tells orchestrators to code trivial fixes directly
  and to run the full `cda-dev` intake; `cda-orchestrator` has no hard "never edit" rule.
- Intake (issues + PRD + plans) runs inside the orchestrator, so one request blocks the next.
- `orch.sh` holds one lock per repo (exit 75) and the skill has no fallback.
- The orchestrator waits in the foreground (`AwaitShell`, `sleep`, `orch.sh | tee`).
- Kit tests publish into the live `CDAMUX_HOME`: fake "PR green" messages reach the
  orchestrator inbox (Refs [#273](https://github.com/chatondearu/ai-dev-kit/issues/273)).

## Goals

- [ ] Every orchestrator session knows its role from turn one (boot prompt delivered, role
      re-injected by hooks on start / restore / compact)
- [ ] The orchestrator never edits, commits or runs verify in a repo; every change goes to a worker
- [ ] A chat request costs the orchestrator one question round + GO, then it dispatches and
      returns within the same turn
- [ ] A second request on a repo with a running `orch.sh` batch is still dispatched
- [ ] Tests never write into the live `CDAMUX_HOME`
- [ ] One source for the routing rule

## Non-goals

- Nudge policy ([#278](https://github.com/chatondearu/ai-dev-kit/issues/278)), run-end
  status (#277), inbox filtering (#276), auto-rebase (#282)
- Global orchestrator ([[PRD-357-global-orchestrator|PRD-357]])
- Cleaning existing junk state under `~/.local/state/cdamux`

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | `cdamux orch new` → first message is the role; "fix X and add Y" → one question round → GO → two workers appear in Fleet; the orchestrator answers "dispatched" and is free |
| Maintainer | A batch `orch.sh` runs on the repo; a new request → worker spawned with the plan as spec instead of waiting |
| Maintainer | Restores an orchestrator after reboot → hooks re-inject the orchestrator routing |
| Kit verify | `bash scripts/verify.sh` inside an orch run → no message reaches the live orchestrator |

## Requirements

### Functional

- [ ] Orchestrator boot prompt passed on the client command line (prompt file), like workers ([#358](https://github.com/chatondearu/ai-dev-kit/issues/358))
- [ ] `CDA_ROLE=orchestrator` in the orchestrator pane command on spawn and respawn (#358)
- [ ] Hook routing returns an orchestrator block when `CDA_ROLE=orchestrator`; routing text has one source ([#359](https://github.com/chatondearu/ai-dev-kit/issues/359))
- [ ] `cda-orchestrator` hard rules, planner-worker intake, exit-75 fallback, end-turn discipline ([#360](https://github.com/chatondearu/ai-dev-kit/issues/360))
- [ ] Harness tests use a temp `CDAMUX_HOME` and no inherited agent id ([#361](https://github.com/chatondearu/ai-dev-kit/issues/361))
- [ ] One no-harness fallback procedure ([#362](https://github.com/chatondearu/ai-dev-kit/issues/362))

### Non-functional

- [ ] Kiss hooks stay local-only, fail-open, no added process spawn
- [ ] No new default wiring in `install.sh` (ADR-001 / ADR-007)
- [ ] `bash scripts/verify.sh` and `cargo test` (cdamux) green

## Related project memory

- ADRs: [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]],
  [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]],
  [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]]
- Plans: [[../plans/PLAN-358-cdamux-orch-boot-prompt-file|PLAN-358]],
  [[../plans/PLAN-359-role-aware-routing|PLAN-359]],
  [[../plans/PLAN-360-orchestrator-hard-rules|PLAN-360]],
  [[../plans/PLAN-361-isolate-tests-cdamux-home|PLAN-361]],
  [[../plans/PLAN-362-merge-no-harness-fallbacks|PLAN-362]]
