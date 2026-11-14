# PRD-334 — cdamux Fleet hygiene and dedicated navigator

- **Status**: approved
- **GitHub**: [#334](https://github.com/chatondearu/ai-dev-kit/issues/334)
- **Date**: 2026-11-14
- **Tags**: #product #architecture

## Problem

1. Finished or stuck orch graphs and workers linger under sessions in Fleet; there is no in-UI way to dismiss them (CLI `cdamux state gc` / `--sweep-orphans` and 14-day retention exist but are easy to miss).
2. After killing a session that owned workers/graphs, a new session (often the same tmux name) can still show that dead work as if it belonged to it — residual gaps after [#307](https://github.com/chatondearu/ai-dev-kit/issues/307).
3. `cdamux sidebar` (join-pane, [#321](https://github.com/chatondearu/ai-dev-kit/issues/321)) injects a Fleet pane into the active session layout. Opening cdamux fullscreen/popup then captures Demux/tmux panels and multiplies ambiguous session chrome. Operators want a navigator that stays beside agent work without living inside those sessions.

## Goals

- [ ] Kill (and equivalent host death) never re-attaches foreign live graphs/workers to a later session of the same name
- [ ] Operator can dismiss finished/orphan under-session rows from Fleet UI via shared finalize/sweep state
- [ ] Dedicated navigator host replaces join-pane sidebar; Alt-P popup unchanged
- [ ] Keybinds are chosen, shown in CDMux chrome (footer + `?`), and documented for the operator

## Non-goals

- ADR-009 resume rewrite
- Foam browser / Alt-P performance epics
- Keeping join-pane sidebar as the primary navigator
- Automatic systemd GC timer
- Further hover polish on the deprecated sidebar (#329) beyond pointing operators at the navigator

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Operator | Kills an orch session, starts a new one with the same name — Fleet must not show the old run as live under it |
| Operator | Sees a finished graph/worker under a session — dismisses it from Fleet without shelling out |
| Operator | Keeps a small navigator always available; switches sessions without splitting agent panes; opens Alt-P when needed |

## Requirements

### Functional

- [ ] Residual reattach gaps after #307 closed (Fleet kill, raw `tmux kill-session`, bind fallback, orphan sweep docs)
- [ ] Fleet dismiss action for finished/orphan under-session rows; same state path as finalize/sweep; protect ADR-009 dormant
- [ ] Navigator = dedicated tmux session (or fixed window), not `join-pane` into agent sessions
- [ ] Deprecate or remove `cdamux sidebar` join-pane once navigator is documented
- [ ] Keybinds implemented and visible (footer and/or `?` help); dogfood lists them

### Keybinds (intake defaults)

| Context | Binding | Action |
| ------- | ------- | ------ |
| tmux (example, not installed by kit) | `Alt-N` (`M-n`) | Toggle / switch client to navigator session `cdamux-nav` (create if missing) |
| tmux | `Alt-P` | Fleet popup — **unchanged** |
| Navigator / Fleet | `j` / `k` | Move selection |
| Navigator / Fleet | `Enter` | Activate/focus selected live session |
| Navigator / Fleet | `q` | Hide/quit navigator (or close popup); do not kill agent sessions |
| Navigator / Fleet | `?` | Help overlay (full keymap) |
| Navigator / Fleet | `d` | Dismiss/finalize selected finished or orphan under-session row (#336) |
| Navigator / Fleet | `x` | Kill selected session (existing finalize-on-kill path) |

Footer must show the primary subset (`j/k · ⏎ · d · x · ? · q`). Dogfood documents the `Alt-N` bind example (replacing the `M-s` sidebar example).

### Non-functional

- [ ] Throwaway tmux server tests for navigator and kill paths
- [ ] No second state store; CDAMUX_HOME only (ADR-011)
- [ ] Verify green for touched crates / kit scripts

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]], [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]], [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]], [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]]
- Prior: PRD-307 finalize stale; PRD-315 cockpit; #259 state GC; #321 sidebar

## Open questions

1. Exact navigator session name (`cdamux-nav` vs configurable) — default `cdamux-nav` unless Settings already has a pattern.
2. Whether `cdamux sidebar` is hard-removed in the same PR as navigator or deprecated with a one-release warning — prefer deprecate-in-docs + stub that prints the navigator command in the same child (#337).

## Success metrics

- Dogfood: kill → recreate same name → no foreign live graph
- Dogfood: dismiss clears a finished under-session row without CLI
- Dogfood: navigator + Alt-P never inject panes into an agent session layout

## Implementation pointer

Children: [[../plans/PLAN-335-fix-cdamux-residual-reattach-after-session-kill|PLAN-335]], [[../plans/PLAN-336-feat-cdamux-fleet-ui-dismiss-under-session-rows|PLAN-336]], [[../plans/PLAN-337-feat-cdamux-dedicated-navigator-session|PLAN-337]]
