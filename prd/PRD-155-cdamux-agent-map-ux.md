# PRD-155 — cdamux agent map: session hierarchy, sub-agents, live graph and steps views, TUI clean

- **Status**: approved
- **GitHub**: [#155](https://github.com/chatondearu/ai-dev-kit/issues/155)
- **Date**: 2026-11-03
- **Tags**: #cdamux #ux #orchestration

## Problem

cdamux lists tmux sessions under projects, and nothing deeper. A forked session is an
unlinked sibling; orch workers are headless subshells with no pane to preview;
Claude Code, Cursor, opencode and pi sub-agents are invisible. The orch graph is an
indented text list at a fixed 72-column width, refreshed by a 100 ms loop that
re-reads files and calls tmux on every tick (#127). The TUI has inverted preview
scrolling, `h`/`l` doing the same thing, a fixed 38/62 split, and no test that renders
a full screen.

## Goals

- [ ] Fleet nests child sessions, orch workers and CLI sub-agents under their parent, each with a preview
- [ ] A drawn DAG view of the orch graph and a per-worker steps view, both live
- [ ] No hot-loop I/O: each source refreshed on its interval and only when it changed
- [ ] Every screen snapshot-tested at 80×24, 120×40, 200×50; a ≤ 5-round clean loop fixes the inconsistencies

## Non-goals

- Web UI parity (it keeps its canvas graph)
- Orch workers in tmux by default (opt-in `ORCH_TMUX=1`; ADR-002 unchanged)
- Fake support for a CLI without a hook surface
- Closing #58 / PRD-41 / PRD-83; extracting cdamux (#56)

## Decisions taken at intake (operator, 2026-11-03)

1. TUI only.
2. `ORCH_TMUX=1` opt-in tmux workers; headless preview = log tail.
3. Sub-agents of every installed CLI (Claude Code, Cursor, opencode, pi) through one local ledger and one adapter per CLI, opt-in via `install.sh --hooks`, fail-open.
4. Layered DAG with box-drawing edges; indented-tree fallback.
5. Polling with mtime caches on one tick; no new dependency.
6. Keymap may change (`h` collapse, `l` expand, `j`/`k` always move, scroll fixed).
7. Clean loop: TestBackend snapshots + throwaway-tmux capture, ≤ 5 rounds.
8. New epic, includes #127.

## Requirements

See [[../plans/PLAN-155-cdamux-agent-map-ux|PLAN-155]] — one child issue per task
(#156–#165) with scope, acceptance and verify command.

## Success metrics

- All child PRs merged; `nix develop -c bash scripts/verify.sh` green on `main`
- A cdamux session running `orch.sh` with `ORCH_TMUX=1` shows its workers as children with live previews, and the Graph view shows the run's DAG with live states

## Links

- Plan: [[../plans/PLAN-155-cdamux-agent-map-ux|PLAN-155]]
- Related: [[PRD-41-cdamux-mission-control-tui|PRD-41]], [[PRD-83-cdamux-fleet-status-preview-agent|PRD-83]]
- ADRs: [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[../foam/decisions/ADR-004-cdamux|ADR-004]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]]; ADR-007 (new, U9)
