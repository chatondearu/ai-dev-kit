# PLAN-258 — Restore orchestrator with its workers

- **Status**: done
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#258](https://github.com/chatondearu/ai-dev-kit/issues/258)
- **Blocked by**: #257
- **Date**: 2026-11-12

## Summary

Extend `cdamux restore` so restoring an orchestrator also restores its workers
(hosts rebuilt, clients resumed by id, parent links kept), building on ADR-009
without absorbing epic #195.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Registry + resume ids; parents before children |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §1 host mandatory for orch/workers; parent/run links |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Throwaway tmux in tests |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Restore orch → workers as a group
- Resume by client id when available; keep parent links

### Out of scope

- Fleet badges (#257 — blocker for UX, but restore can land after)
- Retention GC (#259)
- Full epic #195 scope

## Technical approach

Use agent `role`/`parent` under `CDAMUX_HOME` plus registry records; when
restoring an orchestrator, find children by parent id and restore parents-first
(existing `--all` order extended to “this orch’s subtree”).

## Tasks

- [x] **Subtree restore**: restore orch then its workers with parent links (acceptance: throwaway tmux test)
- [x] **Resume**: clients resume by id when recorded (acceptance: assert resume command / registry id used)
- [x] **Docs**: note orch+workers restore in cdamux docs
- [x] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Partial failure mid-subtree | Keep per-session status lines; exit non-zero if any fail (match today) |

## Test plan

Seams: (1) orch+worker restore order, (2) parent link after restore, (3) resume id.

- [x] `restore_tmux` / new orch-restore test
- [x] Full verify

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made
