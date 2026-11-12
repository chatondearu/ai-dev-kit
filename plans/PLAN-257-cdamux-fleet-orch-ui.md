# PLAN-257 — Fleet/web orch tree, unread badges and ask reply

- **Status**: done
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#257](https://github.com/chatondearu/ai-dev-kit/issues/257)
- **Date**: 2026-11-12

## Summary

Slice 4 UI: Fleet and tailnet web show orchestrators with nested workers, unread
and ask markers, and allow answering an ask from the Fleet composer via
`cdamux msg answer`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §1–§4: parent/role, inbox unread, ask→blocked |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Work in-tree cdamux TUI + web |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Nesting already uses parent links; extend presentation |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Orchestrator glyph + nested workers in Fleet
- ✉N unread, ? pending ask
- Composer reply → `answer`
- Same tree/badges/asks on tailnet web

### Out of scope

- Restore group (#258), retention GC (#259)
- Native wake / MCP

## Technical approach

Read agent role/parent and inbox cursor under `CDAMUX_HOME`; extend fleet_tree /
web dashboard; wire composer send to `cdamux msg send --kind answer`.

## Tasks

- [x] **Tree**: orch glyph + workers nested under parent agent/host (acceptance: fleet unit/snapshot)
- [x] **Badges**: ✉N from unread inbox; ? when pending ask (acceptance: unit tests with temp home fixtures)
- [x] **Reply**: Fleet composer on ask → `answer` msg (acceptance: integration or unit of send path)
- [x] **Web**: same tree/badges/asks in tailnet static UI (acceptance: web/unit or snapshot)
- [x] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Snapshot churn | Update golden fleet snapshots in same PR |

## Test plan

Seams: (1) tree nesting, (2) badge counts, (3) answer send, (4) web parity.

- [x] Temp `CDAMUX_HOME` fixtures
- [x] Fleet snapshots (no new golden needed: glyph/badges covered by unit tests)
- [x] Full verify

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made (none)
