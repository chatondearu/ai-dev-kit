# ADR-004 — cdamux (in-tree until v0, then own repo)

- **Status**: accepted
- **Date**: 2026-10-26
- **Tags**: #architecture #tooling #deployment
- **GitHub**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)

## Context

Operators dogfood **skzv/ccmux** and **agent-manager** for tmux-backed AI coding
agents. Neither fully matches the desired workflow:

- skzv/ccmux: orchestrator that spawns tmux sessions (good), but shallow project
  discovery (one level under `projects.root`), no dynamic `groupBy`, TUI stays open
  in tmux popups after attach.
- agent-manager: excellent spawn/review/quick-prompt and nested logical groups, but
  groups are manual trees (not filesystem `groupBy` like epilande/ccmux), separate
  tmux server (`agentmgr`), no first-class multi-root or folder browser.
- Pickers (MuxPilot, taimux, tmux-agent): great navigation, no spawn/orchestration.

Research and feature matrix: [`cdamux/docs/research.md`](../../cdamux/docs/research.md).

## Decision

1. **New product `cdamux`** — Rust workspace under repo path `cdamux/` (monorepo
   submodule-in-spirit until extractable).
2. **Architecture**: `cdamuxd` daemon (tmux state, HTTP on tailnet optional) +
   `cdamux` CLI (TUI dashboard + `--pick` popup mode that exits after attach).
3. **v0 scope** (see PRD-39 / PLAN-39): multi `projects.roots`, discover modes,
   configurable session `group_by`, new-session flow (agent + directory tree +
   custom path), tailnet API stub for future web UI.
4. **Relationship to ADR-003**: `aitools/` keeps shipping **skzv/ccmux** templates
   and Nix wiring; **cdamux is not enabled** from `install.sh` or default Nix
   modules until v0 dogfood. Optional future `cda.aitools.cdamux` overlay.
5. **Extraction**: When v0 is functional, move `cdamux/` to `github.com/chatondearu/cdamux`
   (or chosen org), replace in-tree path with `git submodule` or vendor pin; update
   this ADR with the new repo URL.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Fork skzv/ccmux (Go) | FSL licence friction; large surface; we want Rust + slimmer v0 |
| Patch-only skzv | Upstream slow on multi-root / picker quit; still Go stack |
| agent-manager only | Missing dynamic groupBy and disk tree; separate tmux server |
| New repo day one | No dogfood in kit context; in-tree lowers friction |

## Consequences

- **Positive**: Single place for research matrix + product intent; agents can bootstrap
  from `cdamux/AGENTS.md`; aligns with “tmux is truth, tool orchestrates”.
- **Negative**: Maintenance of another binary; risk of duplicating ccmux/agent-manager
  unless v0 stays narrow.
- **Constraint**: Do not break ADR-001 (host tools outside kit install path).

## References

- PRD: [`prd/PRD-39-cdamux.md`](../../prd/PRD-39-cdamux.md)
- Plan: [`plans/PLAN-39-cdamux-v0.md`](../../plans/PLAN-39-cdamux-v0.md)
- [[ADR-003-cda-aitools|ADR-003]] cda.aitools companion
