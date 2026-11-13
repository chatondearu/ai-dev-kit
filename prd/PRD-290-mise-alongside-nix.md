# PRD-290 — Mise alongside Nix for daily Node/Rust verify

- **Status**: approved
- **GitHub**: [#290](https://github.com/chatondearu/ai-dev-kit/issues/290)
- **Date**: 2026-11-13
- **Tags**: #product #tooling

## Problem

Agents and operators still fall back to `nix develop -c` for Node/Rust when the
host toolchain is missing or skewed. ADR-013 allows optional **Mise** to pin
Node and Rust for daily loops without replacing flake packages, modules, or
`devShells.default`. The fog after the ADR is not yet implemented: no
`mise.toml`, no documented activate path, no verify entry that prefers Mise
before Nix.

## Goals

- [ ] Root `mise.toml` (+ lock) pins Node and Rust aligned with the current
      flake `devShell` (today: Node **24.x**, Rust **1.95** from nixpkgs).
- [ ] Humans can activate Mise (or direnv+mise); agents must **not** use
      shim-primary PATH.
- [ ] Verify runs: host PATH → one outer Mise (`mise exec` / `mise run`) when
      present → else `nix develop -c bash scripts/verify.sh`.
- [ ] `AGENTS.md` / `nix-develop-shell` notes match the ladder.
- [ ] Flake packages, nix-maid, `cda.aitools`, and `devShells.default` remain.

## Non-goals

- Slimming `devShells.default` (drop node/cargo from the flake).
- Rewriting aitools NixOS/Darwin modules or `install.sh` in Mise.
- Replacing OS CLIs (jq, gh, git, tmux, util-linux) with Mise.

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer with Mise | `mise install` once → `mise activate` / direnv → `bash scripts/verify.sh` uses pinned Node/Rust without `nix develop` |
| Maintainer without Mise | Host tools if OK; else existing `nix develop -c bash scripts/verify.sh` |
| Agent | Follows `nix-develop-shell`: never wrap git/gh; never prefer Mise shims |

## Requirements

### Functional

- [ ] `mise.toml` declares Node + Rust (versions ≈ flake shell).
- [ ] `mise.lock` committed when Mise supports locking for those backends.
- [ ] Document activate / `mise run` (or equivalent) in operator-facing notes
      (`AGENTS.md` and/or short note under `doc/` or skill).
- [ ] Verify oracle updated: prefer host; optional Mise; Nix fallback.
- [ ] No change to `install.sh` skill linking or maid modules.

### Non-functional

- [ ] `bash scripts/verify.sh` (or documented Mise task wrapping it) stays green
      on a machine with either Mise pins or Nix fallback.
- [ ] No shim-primary PATH in docs or scripts.

## Related project memory

- [[../foam/decisions/ADR-013-mise-alongside-nix-devshell|ADR-013]]
- [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]]
- Explore map [#284](https://github.com/chatondearu/ai-dev-kit/issues/284)
- Research: `research/flake-vs-mise-inventory.md`,
  `research/mise-agent-loop-costs.md`

## Open questions

_(none — locked by intake GO 2026-11-13)_

## Success metrics

- Fresh clone with Mise: install tools from `mise.toml`, run verify without
  `nix develop` when Node/Rust come from Mise.
- Fresh clone without Mise: Nix fallback still works.

## Implementation pointer

`plans/PLAN-290-feat-kit-add-mise-toml-and-optional-mise-run-verify-adr-013.md`
