# PLAN-335 — fix residual graph/worker reattach after session kill

- **Status**: in-progress
- **PRD**: [[../prd/PRD-334-cdamux-fleet-hygiene-navigator|PRD-334]]
- **GitHub**: [#335](https://github.com/chatondearu/ai-dev-kit/issues/335)
- **Date**: 2026-11-14

## Summary

Close residual gaps after #307 so killing a session never leaves live graphs/workers re-attachable by session name recreate; document orphan sweep for existing dogfood state.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | State under CDAMUX_HOME; finalize/sweep only |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Do not wipe resumable dormant |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Prefer explicit run id over name-only bind |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux remains the live host DB |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Diagnose remaining reattach paths vs PR #309 (`finalize_session_host`, graph bind order, raw tmux kill)
- Fix gaps; regression tests; dogfood note for `--sweep-orphans`

### Out of scope

- Fleet dismiss UI (#336)
- Navigator (#337)

## Technical approach

Re-read `state_finalize`, `kill_tmux_session` call sites, and `find_orch_graph_for_session`. Cover hosts that die outside cdamux. Prefer fixing shared finalize + bind; keep CLI sweep as the one-shot for existing orphans.

## Tasks

- [x] **Repro matrix**: Fleet `x` vs `tmux kill-session` → recreate same name → assert no foreign live graph/workers (acceptance: written repro + failing test or confirmed green)
- [x] **Close gaps**: any path that leaves `running`/`working` without finalize, or binds live runs by name when `@cdamux_run` is absent on a fresh session (acceptance: unit/integration tests)
- [x] **Docs**: dogfood/architecture note for `cdamux state gc --sweep-orphans [--dry-run]` (acceptance: one checklist item)
- [x] **Verify**: `cargo test` in cdamux (and kit verify if kit scripts touched)

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Operator on pre-#307 binary | Document rebuild/PATH; tests lock behavior on current main |
| Over-finalize dormant | ADR-009 guards in sweep eligibility |

## Test plan

Seams: (1) finalize on kill, (2) graph bind after recreate, (3) orphan sweep dry-run.

- [x] Temp CDAMUX_HOME + throwaway tmux where needed
- [x] Full cdamux test target green (known `home_repair_tmux` fish flake pre-existing)

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made (none — extends ADR-007/011 bind rules)
