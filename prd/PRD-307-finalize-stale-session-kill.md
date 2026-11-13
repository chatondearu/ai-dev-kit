# PRD-307 — Finalize stale orch runs/workers after session kill

- **Status**: approved
- **GitHub**: [#307](https://github.com/chatondearu/ai-dev-kit/issues/307)
- **Date**: 2026-11-13
- **Tags**: #product #cdamux #orchestration

## Problem

After killing a tmux session (and recreating the same name), Fleet still shows
frozen workers and live orch graphs attached to the new session.
`kill_tmux_session` only kills tmux — it does not finalize runs/agents.
`cdamux state gc` only archives terminal statuses past retention — never stuck
`running`/`working`. Session name reuse makes dead hosts look alive again.

## Goals

- [ ] Killing a session from cdamux finalizes related live runs and host agents
      so they cannot re-attach by session name after recreate
- [ ] Fleet does not treat workers with a dead host as live `working` (terminal
      status + clear host; readers already map no-host → dormant)
- [ ] Graph bind prefers `@cdamux_run` / agent run over “any live run with this
      session name”
- [ ] Safe cleanup for already-orphaned dogfood state (`state gc --sweep-orphans`
      with `--dry-run`)
- [ ] Regression tests; verify green; PR closes #307

## Non-goals

- Auto-merge or whole-home destructive wipe
- Changing ADR-009 restore registry semantics (dormant resumable stays protected)
- Alt-P / dashboard perf (#293)
- New parallel state store
- Finalizing inside `kill_tmux_session` used by `restart` (restore must keep agent)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Presses `x` on a live session in Fleet → kill + finalize; recreate same name → no old graph |
| Maintainer | After dogfood chaos, runs `cdamux state gc --sweep-orphans --dry-run` then without dry-run |
| Orchestrator | Restarts a session via `cdamux restart` → agent state preserved (no finalize) |

## Requirements

### Functional

- [ ] Shared `finalize_session_host(session)` marks matching live runs
      `status=error` + `ended_at`, and agents with `host==session` → `error` + clear host
- [ ] TUI permanent kill (`x`) calls finalize after successful kill (not restart path)
- [ ] `find_orch_graph_for_session` prefers non-empty `cdamux_run` before session-name live match
- [ ] `cdamux state gc --sweep-orphans [--dry-run]`: finalize non-terminal agents with dead
      hosts (skip ADR-009 resumable dormant) and live runs whose `session` host is dead
- [ ] Retention GC unchanged for already-terminal entities

### Non-functional

- [ ] Unit tests with temp `CDAMUX_HOME`; tmux tests use throwaway `-L` / `TMUX_TMPDIR` only
- [ ] Host/Mise `cargo test`; no routine `nix develop -c`

## Related project memory

- ADRs: [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]],
  [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]],
  [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]],
  [[../foam/decisions/ADR-004-cdamux|ADR-004]]
- Complements #259 (retention GC), #236 (run-state leaks)

## Open questions

_(None — intake GO 2026-11-13.)_

## Success metrics

- Red loop / fixture: after kill+recreate, no live graph binds via session name
- Dogfood orphans reducible via documented sweep command

## Implementation pointer

`plans/PLAN-307-fix-cdamux-finalize-stale-orch-runs-workers-after-session-ki.md`
