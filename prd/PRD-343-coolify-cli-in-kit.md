# PRD-343 — Coolify CLI in kit (Nix + CLI-first)

- **Status**: approved
- **GitHub**: [#343](https://github.com/chatondearu/ai-dev-kit/issues/343)
- **Date**: 2026-11-14
- **Tags**: #product #tooling #deployment

## Problem

Operators and agents need a first-class Coolify ops path. The kit has a curl/TS
scaffold (`coolify-ops`) but not the official CLI, and no Nix/env wiring. Agents
also lack an explicit preference for CLI over MCP for ops tooling.

## Goals

- [ ] Official `coolify` binary available via `nix build .#coolify-cli`
- [ ] Opt-in `cda.aitools.coolify` installs it when the companion is enabled
- [ ] Simple env-oriented bootstrap documented (token never in the Nix store)
- [ ] `coolify-ops` primary path uses the CLI
- [ ] Kit docs state CLI-first (not MCP) for ops tooling

## Non-goals

- Coolify MCP server
- Wiring via `install.sh`
- Storing API tokens in the flake or world-readable Nix store
- Publishing to nixpkgs upstream in this epic

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| NixOS / nix-darwin operator | Enables `cda.aitools.coolify`, gets `coolify` on PATH after rebuild |
| Agent session | Uses `coolify` with env/context docs; prefers CLI over inventing MCP |
| Cursor plugin user | Runs coolify-ops commands that call the CLI |

## Requirements

### Functional

- [ ] Flake package `coolify-cli` pins a known `coollabsio/coolify-cli` release
- [ ] Module option (off by default) adds the package under `cda.aitools`
- [ ] Docs cover `COOLIFY_BASE_URL` / `COOLIFY_TOKEN` (or thin wrapper) + `coolify context`
- [ ] coolify-ops skills/commands/README use the CLI as the primary path
- [ ] `AGENTS.md` Project-specific + `docs/recommended.md` note CLI-over-MCP

### Non-functional

- [ ] No secrets in git or Nix derivations
- [ ] Companion remains opt-in (ADR-001 / ADR-003)
- [ ] `bash scripts/verify.sh` still passes

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]], [[../foam/decisions/ADR-003-cda-aitools|ADR-003]]
- Children: [#344](https://github.com/chatondearu/ai-dev-kit/issues/344), [#345](https://github.com/chatondearu/ai-dev-kit/issues/345)

## Open questions

None — intake GO 2026-11-14 locked packaging location, env model, coolify-ops
reorientation, and doc placement.

## Success metrics

- Operator can `nix build .#coolify-cli` and enable companion option
- Agent-facing docs point at CLI, not MCP, for Coolify/ops

## Implementation pointer

- [[../plans/PLAN-344-feat-aitools-package-coolify-cli-opt-in-cda-aitools-coolify|PLAN-344]]
- [[../plans/PLAN-345-docs-coolify-point-coolify-ops-at-cli-note-cli-first-prefere|PLAN-345]]
