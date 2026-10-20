# PRD-33 — cda.aitools companion (experimental)

- **Status**: shipped
- **GitHub**: [#33](https://github.com/chatondearu/ai-dev-kit/issues/33)
- **Date**: 2026-10-20
- **Tags**: #architecture #tooling #deployment

## Problem

Operators want versioned, multi-machine setup for agent terminal sessions (tmux/ccmux) and related host wiring, without forcing apps into `install.sh` or breaking the kit-only portable path (ADR-001). Configs often live outside `~/` (e.g. `/cda/...`).

## Goals

- [ ] Opt-in companion under `aitools/` with Nix modules (darwin + nixos)
- [ ] Relocatable root via `CDA_AITOOLS_HOME` / module `configRoot`
- [ ] tmux/ccmux templates + `installMethod` auto|nix|brew|none
- [ ] Bootstrap prompt for other machines (not a flattened project skill)
- [ ] Orch worktree cleanup command
- [ ] ADR-001 amended + ADR for cda.aitools; status experimental until dogfood

## Non-goals

- `install.sh` installing apps
- Automatic ccmux ↔ orch worktree bridge
- Stable README primary path before dogfood on ≥2 hosts
- Packaging Cursor/Claude in the companion

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer (nix-darwin) | Enable `cda.aitools` with `configRoot = /cda/...` |
| Friend on Linux/NixOS | Same modules; brew not required |
| Non-Nix machine | Agent follows `aitools/prompts/bootstrap.md` |

## Requirements

### Functional

- [ ] #34 — ADR-001 amend + ADR cda.aitools
- [ ] #35 — skeleton + nixos/darwin modules + ENV
- [ ] #36 — tmux/ccmux templates + installMethod (Blocked by #35)
- [ ] #37 — bootstrap prompt
- [ ] #38 — orch worktree cleanup

### Non-functional

- [ ] Kit `install.sh` unchanged regarding apps
- [ ] Experimental labeling until dogfood
- [ ] English artifacts

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]], [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]]
- Grill accord 2026-10-20 (chat)

## Open questions

_(none — grill accord)_

## Success metrics

- Children acceptance criteria met; PRs CI-green; human merges

## Implementation pointer

Per-child `plans/PLAN-<id>-*.md`
