# PRD-72 — cdamux worktrees, spawn branch, session preview chrome

- **Status**: approved
- **GitHub**: [#72](https://github.com/chatondearu/ai-dev-kit/issues/72)
- **Parent epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Date**: 2026-10-27
- **Tags**: #product #tooling #ux

## Problem

PRD-41 shipped Fleet with pane **preview** and a **spawn modal**, but operators still
lack **git context at a glance** (repo, worktree vs main checkout, branch, which agent
is running, how long the session has been up). Creating work on a **feature branch**
or **linked worktree** still means leaving cdamux for shell `git worktree` commands—
unlike ccmux/epilande spawn flows.

Partial phase-A code exists (`SessionChrome` in preview); spawn worktrees and rich
agent telemetry are not specified end-to-end.

## Goals

- [ ] **Preview chrome**: bordered header above captured pane with repo, worktree
  label, branch, active agent, session uptime; placeholders for context % and
  task/sub-agent counts until a real data source exists.
- [ ] **Spawn — branch**: choose **base branch** (default: current or remote default)
  when creating a session on an existing repo path.
- [ ] **Spawn — worktree**: optional mode to `git worktree add` under a **configured**
  root (e.g. `~/.orch/worktrees/` or `cdamux` config); session cwd is the new worktree.
- [ ] **Agent label**: persist `@cdamux_agent` on spawn (and read legacy `@ccmux_agent`).
- [ ] **Return to Fleet**: document tmux hooks (`@cdamux_return`, Alt+u) for reopening
  dashboard after agent exit (aitools template).
- [ ] Update `cdamux/docs/research.md` worktree row when shipped.

## Non-goals

- Automatic creation or sync with **orch-auto** / Kanban worktrees (ADR-003)
- Curated logical groups (agent-manager style) in this PRD
- Parsing proprietary agent UIs for token context without a documented API
- Enabling cdamux from `install.sh` or default Nix (ADR-004 / ADR-003 still apply)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Select session in Fleet → read chrome → see `ai-dev-kit` on `feat/foo` worktree, `cursor` agent, 2h uptime → attach |
| Feature work | `n` → pick repo → base branch `main` → new worktree `…/worktrees/issue-72` → spawn cursor-agent |
| Shell-only | Same flow with **shell only** on worktree path for manual git |
| Popup user | Alt+p dashboard; chrome visible in Preview before attach |

## Requirements

### Functional — preview chrome

- Chrome refreshes when Fleet selection changes (same session as preview tail).
- Fields: `repo` (toplevel basename), `worktree` (`main` vs linked worktree name),
  `branch`, `agent`, `uptime`, `context`, `tasks`.
- `context` / `tasks` show `n/a` until phase C (daemon/agent); no fake numbers.
- Chrome visible only on **Preview** sidecar tab.

### Functional — spawn

- Modal retains path sources: running cwd | discovered repo | custom path.
- **Branch**: list local branches for selected repo; default from config or current HEAD.
- **Worktree mode** (optional toggle): target directory + `git worktree add -b <branch>`.
- Fail with clear CLI/TUI message if git command fails (dirty tree, existing path).
- Reuse existing layout options (single / h / v split) and agent picker with
  **preferred agent** (`[agents].default` on PATH, else first available).

### Functional — session lifecycle

- `x` kill, `f` fork, `m` rename (shipped separately; remain compatible with chrome).
- Fork creates new session name suffix (`c-repo-2`) same cwd semantics as today.

### Non-functional

- Git subprocess calls must not block TUI > 200ms p95 on warm cache (cache per path
  for branch/worktree in daemon or in-process TTL).
- No new required dependencies beyond `git` and `tmux`.

## Related project memory

- [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] — no auto-bridge orch ↔ session tool worktrees
- [[../foam/decisions/ADR-004-cdamux|ADR-004]] — cdamux in-tree, Rust, tmux orchestration
- [[../prd/PRD-41-cdamux-mission-control-tui|PRD-41]] — Fleet / spawn modal baseline (shipped)
- [[../prd/PRD-39-cdamux|PRD-39]] — v0 orchestrator scope
- Plan: [[../foam/archive/plans/PLAN-72-cdamux-worktree-session-chrome|PLAN-72]] (phased checklist, archived)
- Research: [`cdamux/docs/research.md`](../cdamux/docs/research.md)

## Open questions

1. Default worktree root: single config key `worktrees.root` vs reuse orch path env?
2. Should fork duplicate branch name or always prompt?
3. Which agent exposes task/sub-agent count first (Cursor CLI, Claude, MCP)?
4. Cache chrome in `cdamuxd` JSON API for tailnet web (#45)?

## Success metrics

- Operator can name repo + branch + worktree for any selected session without `tmux attach`.
- One successful spawn into a new worktree from the modal on neo-nix dogfood.
- Zero regressions on `cdamux --pick` contract (PRD-41).

## Implementation pointer

Phased checklist: [`foam/archive/plans/PLAN-72-cdamux-worktree-session-chrome.md`](../foam/archive/plans/PLAN-72-cdamux-worktree-session-chrome.md)

Phase A (chrome + agent label) partially landed on `main`; complete refresh/caching and docs before marking shipped.
