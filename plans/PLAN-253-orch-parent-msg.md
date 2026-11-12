# PLAN-253 — Send parent cdamux msg on worker/run completion

- **Status**: done
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#253](https://github.com/chatondearu/ai-dev-kit/issues/253)
- **Blocked by**: #252
- **Date**: 2026-11-12

## Summary

When an orch run sits under a parent orchestrator (`CDAMUX_AGENT_ID`), emit
`cdamux msg` for each worker terminal state and once at run end with a `ref` to
the integrator report (ADR-011 §5).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §4–§5 message kinds; parent notify on done/blocked/error + run end |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | No-op without cdamux / without parent id |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Per-worker `done` / `blocked` / `error` → `cdamux msg` to parent
- Run-end message with `ref` to integrator report
- No-op without cdamux or parent

### Out of scope

- Hard cut / Fleet readers (#252)
- Native wake
- Slice 4 UI badges

## Technical approach

Hook orch worker completion and run aggregate finish to call `cdamux msg send`
toward the parent agent id from the environment / run record.

## Tasks

- [x] **Worker terminal msgs**: on done/blocked/error, send msg to parent when `CDAMUX_AGENT_ID` set (acceptance: stub orch test asserts msg file / CLI invocation)
- [x] **Run-end msg**: one message with `ref` to integrator report (acceptance: test)
- [x] **No-op**: missing cdamux or parent → silent (acceptance: test)
- [x] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Duplicate msgs on retry | Idempotent kind+worker id or document at-most-once best effort |

## Test plan

Seams: (1) worker terminal → msg, (2) run end → msg+ref, (3) no-op paths.

- [x] Temp `CDAMUX_HOME` inbox assertions
- [x] Full verify

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made
