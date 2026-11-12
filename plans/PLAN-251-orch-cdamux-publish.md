# PLAN-251 — Publish orch run/nodes/agents via cdamux state

- **Status**: implemented
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#251](https://github.com/chatondearu/ai-dev-kit/issues/251)
- **Date**: 2026-11-12

## Summary

Slice 3 publish path: `orch.sh` and `graph-node.sh` write run, nodes and worker
Agents through `cdamux state` under `CDAMUX_HOME`, with sub-run linkage when
`CDAMUX_AGENT_ID` is set, and silent no-op without `cdamux` (ADR-001 / ADR-011 §5).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §5 publish via CLI; sub-run when parent agent id set; no-op without binary |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Kit must not require cdamux; pipeline works without it |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Graph node commands stay; storage moves behind state CLI |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | orch-auto / ready-pickup behaviour unchanged aside from state location |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `orch.sh` → `cdamux state` for run / nodes / worker Agents
- Sub-run when `CDAMUX_AGENT_ID` set
- `graph-node.sh` thin wrapper calling `cdamux state`
- Keep in-repo work artefacts

### Out of scope

- Deleting `status.json` / graph writers ( #252 )
- Parent `cdamux msg` on completion ( #253 )
- Fleet UI / restore (slice 4)

## Technical approach

Add a small orch helper that shells out to `cdamux state` (fail-open). Map today's
progress/graph updates onto agent + run JSON under `CDAMUX_HOME` while still
writing the existing repo files until #252 hard-cuts them.

## Tasks

- [x] **Publish helper**: orch writes run/nodes/worker Agents via `cdamux state` when available (acceptance: temp `CDAMUX_HOME` shows files after a stubbed orch phase)
- [x] **Sub-run**: with `CDAMUX_AGENT_ID`, run records parent agent (acceptance: unit/integration assert)
- [x] **graph-node.sh**: same CLI surface; implementation calls `cdamux state` (acceptance: existing graph-node tests adapted or new stub tests)
- [x] **No-op path**: without `cdamux` on PATH, orch completes as today (acceptance: test with PATH stripped of cdamux)
- [x] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Dual write until #252 | Explicit: publish to CDAMUX_HOME now; leave old writers for hard-cut child |
| Schema drift vs Fleet | Reuse slice 1 agent/run shapes from ADR-011 |

## Test plan

Seams: (1) publish with cdamux, (2) no-op without, (3) sub-run parent, (4) graph-node commands.

- [x] Temp `CDAMUX_HOME` orch stub run
- [x] PATH without cdamux
- [x] `nix develop -c bash scripts/verify.sh`

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made (ADR-011 amended: dual-write until #252)
