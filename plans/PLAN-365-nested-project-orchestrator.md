# PLAN-365 — feat(cdamux): project orchestrator as child of the global orchestrator

- **Status**: done
- **PRD**: [[../prd/PRD-357-global-orchestrator|PRD-357]] (slice 2)
- **GitHub**: [#365](https://github.com/chatondearu/ai-dev-kit/issues/365)
- **Date**: 2026-11-16

## Summary

The global orchestrator starts a project orchestrator as its child: `role=orchestrator`,
`parent` = global id, its own run and workers. Workers report to it; it reports to the
global one with `cdamux msg`. Restore brings the chain back parents-first.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] | §3: depth ≤ 2 (global → project → workers); a project orchestrator cannot spawn orchestrators |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | `parent` / `run` in agent state; state API only |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Registry keeps the parent link; restore parents-first |
| `cdamux/AGENTS.md` | Tests on a throwaway tmux server only |

ADR-015 acceptance is #367's job — do not edit its status here. Merge after #367.

## Scope

### In scope

- `cdamux/crates/cdamux-cli/src/main.rs`: `orch new --child --path <path|project>`, run by
  the global orchestrator; parent = caller (`CDAMUX_AGENT_ID`). `--path` also accepts a
  project name via the #363 resolver. `--child` with `--global` is an error.
- `cdamux/crates/cdamux-core/src/orch.rs`: child orchestrator gets `parent`, fresh run,
  project boot prompt; refuse when the caller is not `scope=global` (depth rule)
- `cdamux/crates/cdamux-core/src/restore.rs`: subtree restore from the global root
  includes project orchestrators and their workers (extend `restore_subtree` tests)
- Fleet nesting check (`fleet_tree.rs`) — likely already works via `parent`; add a test
- Doc line in `skills/core/cda-orchestrator/SKILL.md` § Global only if #367 merged first
  (else leave for a follow-up)

### Out of scope

- Depth > 2, cross-project worker moves, ADR-015 status

## Technical approach

1. Reuse `--repo` resolution from #363 (project name or path) for the child's cwd.
2. `spawn_orchestrator` with `parent = caller_id` when caller is the global orchestrator
   (`CDAMUX_AGENT_ID` → agent `scope=global`); new run id; `CDA_ROLE=orchestrator`.
3. `worker spawn` from the child already sets `parent` = child (unchanged); verify
   `cdamux msg --to parent` from a worker resolves to the child.
4. Restore: `restore_subtree(global)` walks `parent` links two levels.

## Tasks

- [x] CLI surface + depth guard + error messages
- [x] Child state (`role`, `parent`, `run`, no `scope=global`)
- [x] Restore subtree two levels + unit test
- [x] Throwaway-tmux test: global → child orch → worker; worker msg reaches child
- [x] Fleet nesting test
- [x] `cargo test` in `cdamux/`; `bash scripts/verify.sh`; PR `Closes #365`
- [x] `cda-orchestrator` § Global: `--child` line (#367 already merged)

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Overlap with #366 in `worker.rs` / `lib.rs` | Rebase on main before PR |
| Global orchestrator absent (caller is a project orch) | Clear refusal naming the depth rule |

## Test plan

- [x] `cargo test`
- [x] `bash scripts/verify.sh`
