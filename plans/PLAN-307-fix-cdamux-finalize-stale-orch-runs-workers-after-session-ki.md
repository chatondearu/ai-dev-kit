# PLAN-307 — Finalize stale orch runs/workers after session kill

- **Status**: done
- **PRD**: [[../prd/PRD-307-finalize-stale-session-kill|PRD-307]]
- **GitHub**: [#307](https://github.com/chatondearu/ai-dev-kit/issues/307)
- **Date**: 2026-11-13

## Summary

On permanent session kill, finalize related `$CDAMUX_HOME` agents/runs; prefer
`@cdamux_run` when binding graphs; add `--sweep-orphans` (+ dry-run) for existing
orphans without touching resumable dormant registry entries.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Single store under CDAMUX_HOME; status set includes `error`; no parallel store |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Sweep must keep resumable dormant; restart kill must not finalize |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Throwaway tmux in tests only |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux remains truth for live sessions |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Finalize-on-permanent-kill helper + TUI wire-up
- Graph bind order fix
- `state gc --sweep-orphans [--dry-run]`
- Tests + short operator note in architecture.md

### Out of scope

- Finalize inside raw `kill_tmux_session` (breaks restart)
- Fleet UI redesign
- Automatic GC timer

## Technical approach

1. New core helper (e.g. `state_finalize`) that sets run `error`/`ended_at` and
   agent `error` + empty host for a given session name; shared by TUI kill and sweep.
2. Flip `find_orch_graph_for_session` to try `cdamux_run` first.
3. Extend `StateCommands::Gc` with `--sweep-orphans` / `--dry-run`; retention pass
   still runs after (or dry-run prints only).

## Tasks

- [x] **Finalize helper**: given a session name, mark matching live runs and host
      agents terminal (`error`); acceptance: unit test temp home, live graph gone
- [x] **TUI kill**: after successful permanent kill + registry remove, call finalize;
      acceptance: restart path unchanged (no call)
- [x] **Graph bind**: prefer `cdamux_run` over session-name live match; update unit test
- [x] **Sweep orphans**: CLI flags + finalize dead-host non-terminal agents (skip
      resumable dormant) and live runs with dead session host; dry-run prints only
- [x] **Docs**: one-shot cleanup line in `cdamux/docs/architecture.md`
- [ ] **Verify**: `cargo test` in cdamux + kit verify

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Finalize on restart kills restore | Only call from TUI permanent kill / explicit sweep |
| Sweep archives live agents | Dead-host check + ADR-009 skip + dry-run |
| Session name reuse mid-sweep | Finalize clears host so recreate cannot revive old agent |

## Test plan

Seams: (1) `finalize_session_host`, (2) `find_orch_graph_for_session` order,
(3) `sweep_orphans` eligibility / dry-run.

- [x] Temp `CDAMUX_HOME` fixtures (no live tmux required for unit tests)
- [ ] Full cdamux `cargo test` + kit verify

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR only if a new decision was made
