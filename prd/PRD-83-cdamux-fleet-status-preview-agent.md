# PRD-83 — cdamux Fleet status, preview capture, agent tag

- **Status**: approved
- **GitHub**: [#83](https://github.com/chatondearu/ai-dev-kit/issues/83)
- **Parent epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Date**: 2026-10-28
- **Tags**: #product #tooling #ux

## Problem

Fleet session status stays green (Active) for every session because `pane_idle_time`
is empty on some tmux builds and is parsed as idle=0. Claude Code panes with
`history_size=0` yield an empty preview when `capture-pane` uses `-E -1`. The
running agent is stored in `@cdamux_agent` and shown in Preview chrome, but not
on session rows in the tree.

## Goals

- [ ] Session classification remains meaningful when `pane_idle_time` is missing
- [ ] Preview sidecar shows pane content for zero-history agents (Claude)
- [ ] Fleet session rows show the agent tag from tmux user options
- [ ] Regression tests cover empty idle and zero-history capture paths

## Non-goals

- Parsing proprietary agent UIs for sub-agent / context % (stay `n/a` per PRD-72)
- Rewriting Notes or Review sidecar behaviour
- Changing orch graph format or execution (ADR-005 display-only)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Open Fleet → idle sessions not all green; Claude session preview shows UI; row shows `claude` |
| Operator | Scan list for which agent runs without opening Preview |

## Requirements

### Functional

- [ ] Empty/missing `pane_idle_time` must not force Active via idle=0 alone
- [ ] Prefer a documented fallback activity signal when idle is unavailable
- [ ] `capture_pane_text` must return visible pane content when history is empty
- [ ] Session rows show agent from `@cdamux_agent` (legacy `@ccmux_agent`); missing → placeholder
- [ ] Children: #84, #85

### Non-functional

- [ ] No new dependencies beyond tmux/git already required
- [ ] `cargo test` in `cdamux/` green

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]], [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]]
- Prior: [[PRD-72-cdamux-worktree-session-chrome|PRD-72]] chrome agent field

## Open questions

1. Exact fallback when `pane_idle_time` unsupported — `pane_last_activity` vs treat as Idle/unknown (worker chooses smallest correct fix).

## Success metrics

- Live dogfood: Claude session preview non-empty; mixed idle/active colours; agent tags on rows

## Implementation pointer

- `foam/archive/plans/PLAN-84-cdamux-idle-preview.md`
- `foam/archive/plans/PLAN-85-cdamux-agent-tag.md`
- Graph: `plans/graph/epic-83.json`
