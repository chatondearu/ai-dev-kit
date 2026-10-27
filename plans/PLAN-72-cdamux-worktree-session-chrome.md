# PLAN-72 — worktrees, base branch, session preview chrome

- **Status**: in progress
- **GitHub**: [#72](https://github.com/chatondearu/ai-dev-kit/issues/72)
- **PRD**: [PRD-72](../prd/PRD-72-cdamux-worktree-session-chrome.md)
- **Parent epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **ADR**: [ADR-003](../foam/decisions/ADR-003-cda-aitools.md) — no auto-bridge to `.orch/worktrees/`; explicit paths only

## Problem

See PRD-72. Operators need git-aware Fleet preview and spawn-time worktree/branch
choice without orch auto-bridge.

## Phase A — Preview chrome

- [x] Bordered header above Preview pane: repo, worktree label, branch, agent, uptime
- [x] Set `@cdamux_agent` on spawn for agent id (shell → `shell`)
- [x] Chrome cache / refresh policy (avoid git subprocess every 800ms)
- [x] `context` / `tasks` placeholder documented in PRD until phase C
- [ ] Dogfood sign-off on issue #72

## Phase B — Spawn: branch + worktree

- [x] Config: `worktrees.root` (optional) in `config.toml`
- [x] Spawn modal: branch picker + worktree toggle
- [x] `git worktree add` integration with error surfacing in modal
- [x] `cdamux worktree list` or extend `projects` output
- [x] Docs: explicit opt-in vs orch worktrees

## Phase C — Rich agent telemetry

- [ ] cdamuxd poll hooks per agent when API known
- [ ] Chrome: context %, sub-agent/task counts
- [ ] Tailnet API fields (optional, #45)

## Verify

```bash
cd cdamux && cargo test && cargo build -p cdamux-cli
# Fleet → select session → Preview shows session chrome
```
