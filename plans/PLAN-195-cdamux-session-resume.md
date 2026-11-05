# PLAN-195 — cdamux session resume

- **Status**: in progress
- **PRD**: [[../prd/PRD-195-cdamux-session-resume|PRD-195]]
- **GitHub**: [#195](https://github.com/chatondearu/ai-dev-kit/issues/195)
- **Date**: 2026-11-05

## Summary

Four Task subagents, one worktree + branch + PR each, stacked on this PRD/plan PR.
R1 lays the registry; R2 and R3 run in parallel on R1; R4 runs on R3.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | no auto-merge; nothing enabled by plain `install.sh` |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux is the truth for what runs — the registry is never read to decide liveness (amended by ADR-008) |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | links stay tmux options; hooks share `cdamux/hooks/ledger.js`, opt-in `--cdamux-hooks`, fail-open, local only |

## Tasks

The full contract of each task is the child issue body.

| Id | Child | Task | Depends |
| -- | ----- | ---- | ------- |
| R1 | #196 | Restore registry module, record on spawn/fork, forget on close, rename, Claude `--session-id` pre-assign, ADR-008 | — |
| R2 | #197 | `SessionStart` id-capture hooks (Claude, Cursor, opencode) + `install.sh --cdamux-hooks` wiring next to kiss | R1 |
| R3 | #198 | Per-agent `resume` template + `cdamux restore <name>|--all` | R1 |
| R4 | #199 | Fleet dormant rows (restore / forget keys) + `cdamux restart [--all] [--force]` | R3 |

Wave 1: R1. Wave 2: R2, R3. Wave 3: R4.

- [ ] R1 · [ ] R2 · [ ] R3 · [ ] R4

## Risks

- Resume flags for Cursor (`--resume=`) and opencode (`--session`) come from their docs, not a live run.
- Without the hooks, a Claude session that ran `/clear` resumes the pre-clear conversation (pre-assigned id).
- `install.sh` already owns a Claude `SessionStart` entry (kiss): R2 must add, never replace.
- `restart --all` kills agents: busy-skip by default, home session never killed, current client last.
- tmux tests on throwaway servers only (`cdamux/AGENTS.md` § Testing against tmux).
