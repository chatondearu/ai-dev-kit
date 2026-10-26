# PRD-39 — cdamux (agent session orchestrator)

- **Status**: shipped (v1 in-tree; dashboard depth → [[PRD-41-cdamux-mission-control-tui|PRD-41]] / #58)
- **GitHub**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Date**: 2026-10-26
- **Tags**: #architecture #tooling

## Problem

Managing many AI agent sessions in tmux requires juggling tools: skzv/ccmux for
spawning and remote notes, agent-manager for review and quick prompts, epilande-style
pickers for popup navigation and `groupBy`, and ad-hoc scripts for multi-root layouts
like `/hdd/dev/**/repo`. No single tool combines orchestration, project discovery,
dynamic grouping, picker UX, tailnet access, and a path to browser UI.

## Goals

- [x] In-tree Rust workspace at `cdamux/` with `cdamuxd` + `cdamux` CLI
- [x] **Multi-root** project config and **discover** modes (`flat`, `depth`, `git_only`)
- [x] Session list **group_by**: `project | cwd | agent | group_path | none` (epilande parity)
- [x] **New session** flow: pick agent, browse directory tree or custom path, optional group
- [x] **`cdamux --pick`**: attach/switch then **exit** (tmux `display-popup -E` friendly)
- [x] Daemon: classify tmux sessions (active / idle / needs_input), Unix socket + optional HTTP on tailnet
- [x] Document research matrix and competitor attribs in `cdamux/docs/`
- [ ] Extract to standalone repo when v0 dogfood passes (ADR-004) — tracked after maintainer sign-off (#48)

## Non-goals (v0)

- Replacing skzv/ccmux or agent-manager in production Nix config
- `install.sh` integration
- Full parity with 12 agent CLIs (start with Cursor + OpenCode + shell; extensible config)
- Native iOS app (Mosh + TUI or web on tailnet first)
- Moshi hook reimplementation (reuse existing stack where possible)
- Hunk review UI (agent-manager parity → v1)
- Teep-style file rail (v1+)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Popup `M-p` → pick session → pane focuses agent; popup closes |
| Multi-repo layout | Roots `/hdd/dev` + `/home/chaton/etc/nixos`; discover `git_only` |
| Fleet | Several agents; group list by `cwd` or `project` without manual group tree |
| Remote | Laptop lists sessions on always-on host via tailnet API (read + spawn later) |
| Future | Browser dashboard on tailnet for status + send (v0.2) |

## Requirements

### Functional (v0)

- [x] Read `~/.config/cdamux/config.toml` (`projects.roots[]`, `discover`, `group_by`, `agents.default`)
- [x] `cdamuxd` polls tmux (default socket; configurable), exposes local Unix API
- [x] `cdamux list` / `cdamux attach` / `cdamux spawn --agent --path`
- [x] `cdamux --pick` interactive picker (ratatui or similar)
- [x] Nested tmux: `switch-client` + quit TUI (not return to dashboard)
- [x] `cdamux doctor` (tmux, tailscale optional, agent binaries on PATH)

### Functional (v0.2)

- [x] HTTP API on tailnet bind (list sessions, read pane tail, send keys)
- [ ] Minimal static web UI consuming API (#45)
- [x] SSH/Mosh documented attach path (no custom mobile app) — see `cdamux/docs/architecture.md`

### Non-functional

- [ ] MIT OR Apache-2.0 licence (Rust ecosystem friendly) — add at extract time
- [x] English code comments and operator docs
- [x] No telemetry by default
- [x] Fits ADR-001: optional host tool, not kit core

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]], [[../foam/decisions/ADR-003-cda-aitools|ADR-003]], [[../foam/decisions/ADR-004-cdamux|ADR-004]]
- Research: [`cdamux/docs/research.md`](../cdamux/docs/research.md)
- Architecture: [`cdamux/docs/architecture.md`](../cdamux/docs/architecture.md)

## Open questions

- Shared tmux socket vs dedicated `-L cdamux` (agent-manager style isolation)?
- Hook-based status vs pane heuristics first?
- Name on crates.io: `cdamux` vs `cda-mux` binary only?

## Success metrics (v0)

- Dogfood on maintainer machine: spawn 3 projects from 2 roots, pick via popup, no stuck overlay
- One other host can `curl` daemon health over tailnet (v0.2)

Evidence: `cdamux/docs/dogfood.md` sign-off (issue #48).
