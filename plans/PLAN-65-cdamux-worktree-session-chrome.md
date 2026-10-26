# PLAN-65 — worktrees, base branch, session preview chrome

- **Status**: in progress
- **Parent**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **PRD**: follow-up to [PRD-41](../prd/PRD-41-cdamux-mission-control-tui.md) (v2.1)
- **ADR**: [ADR-003](../foam/decisions/ADR-003-cda-aitools.md) — no auto-bridge to `.orch/worktrees/`; explicit paths only

## Problem

Operators need **ccmux / agent-manager parity** on git context: which repo, worktree,
branch, and agent when spawning and when reading the Fleet preview. Sub-agent/task
counts need a stable data source (daemon or agent hooks) — not invented in the CLI.

## Goals (phased)

### Phase A — Preview chrome (this slice)

- [x] Bordered header above Preview pane: repo, worktree label, branch, agent, uptime
- [x] Set `@cdamux_agent` on spawn for agent id (shell → `shell`)
- [ ] Refresh chrome on session selection (same cadence as preview tail)
- [ ] `context` / `tasks` line: placeholder until daemon/agent API exists

### Phase B — Spawn: branch + worktree

- [ ] Spawn modal: **base branch** (git default branch or current) + optional new branch name
- [ ] Modes: **existing path** | **git worktree** under `.orch/worktrees/<issue-slug>` or configurable root
- [ ] `cdamux worktree list` / reuse `discover` for linked worktrees
- [ ] Document explicit opt-in; no silent merge with orch worktrees (ADR-003)

### Phase C — Rich agent telemetry

- [ ] cdamuxd poll: parse agent-specific status (Cursor tasks, Claude tokens, etc.) when documented
- [ ] Chrome fields: context %, active sub-agents count
- [ ] Optional tailnet API fields for mobile

## Non-goals

- Replacing `orch-auto.sh` worktree creation
- ccmux-compatible `@ccmux_*` beyond read fallback

## Verify

```bash
cd cdamux && cargo test && cargo build -p cdamux-cli
# Fleet → select session → Preview shows chrome block
```
