# PLAN-155 — cdamux agent map with a fleet of subagents

- **Status**: in-progress
- **PRD**: [[../prd/PRD-155-cdamux-agent-map-ux|PRD-155]]
- **GitHub**: [#155](https://github.com/chatondearu/ai-dev-kit/issues/155)
- **Date**: 2026-11-03

## Summary

Ten Task subagents, one worktree + branch + PR each, stacked. Progress is recorded
with `graph-node.sh` so the run is visible in the cdamux Orch graph sidecar
(dogfood of U5/U6).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | headless orch stays default; no auto-merge |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux is the source of truth; cdamux reads it directly |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | graph files are the contract (`current.json`, `status.json`) |

## Tasks

The full contract of each task is the child issue body.

| Id | Child | Task | Model | Depends |
| -- | ----- | ---- | ----- | ------- |
| U0 | #156 | Full-screen render harness + golden snapshots | sonnet | — |
| U1 | #157 | Session links model, nested Fleet rows, `PreviewSource` | opus | U0 |
| U2 | #158 | `ORCH_TMUX=1` linked worker sessions, `@cdamux_state` | opus | U1 |
| U3 | #159 | Sub-agent ledger + Claude Code adapter | opus | U1 |
| U4 | #160 | Cursor / opencode / pi adapters (or documented gaps) | sonnet | U3 |
| U5 | #161 | Drawn DAG graph view | opus | U0 |
| U6 | #162 | Steps view | sonnet | U5 |
| U7 | #163 | One refresh scheduler, mtime caches (closes #127) | sonnet | U0 |
| U8 | #164 | TUI clean loop (≤ 5 rounds) | opus | U1–U7 |
| U9 | #165 | ADR-007 + docs | sonnet | U2, U3 |

Wave 1: U0, U5, U7 (U5/U7 add their snapshots once U0 lands). Wave 2: U1, U6. Wave 3: U2, U3. Wave 4: U4, U9. Wave 5: U8.

- [ ] U0 · [ ] U1 · [ ] U2 · [ ] U3 · [ ] U4
- [ ] U5 · [ ] U6 · [ ] U7 · [ ] U8 · [ ] U9

## Risks

- U1, U5, U7, U8 all touch `tui.rs` / `fleet_draw.rs`: waves are stacked and rebased; U8 runs last.
- opencode and pi are not installed on the maintainer's Mac: U4 tests use recorded payloads.
- Hook adapters run on every agent session: fail-open, < 50 ms, local only.
