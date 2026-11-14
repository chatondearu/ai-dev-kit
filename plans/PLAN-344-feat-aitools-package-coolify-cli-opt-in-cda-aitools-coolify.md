# PLAN-344 — feat(aitools): package Coolify CLI + opt-in cda.aitools.coolify

- **Status**: in-progress
- **PRD**: [[../prd/PRD-343-coolify-cli-in-kit|PRD-343]]
- **GitHub**: [#344](https://github.com/chatondearu/ai-dev-kit/issues/344)
- **Date**: 2026-11-14

## Summary

Pin the official Coolify CLI as a flake package and expose it via opt-in
`cda.aitools.coolify`, with env-friendly bootstrap docs (token host-side only).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Do not wire through `install.sh`; companion / flake only |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | Opt-in module; never enable by default from kit install |
| [[../foam/decisions/ADR-013-mise-alongside-nix-devshell|ADR-013]] | Prefer host/Mise for verify; do not wrap routine commands in `nix develop -c` |

## Scope

### In scope

- Flake output `packages.<system>.coolify-cli`
- Nix option `cda.aitools.coolify` (enable + installMethod if needed)
- Wire package into `module-core.nix` systemPackages when enabled
- Document env / context bootstrap in `aitools/README.md` (and short package comment)

### Out of scope

- coolify-ops rewrite (PLAN-345)
- MCP
- Committing API tokens

## Technical approach

Mirror `graphify`: `aitools/nix/coolify-cli-package.nix` (prefer pinned GitHub
release binary via `fetchurl` + `installBin`, or `buildGoModule` if binaries are
awkward). Export from `flake.nix`. Add options in `aitools/nix/common.nix`;
consume in `module-core.nix` like graphify. Optional thin wrapper only if the
upstream CLI lacks usable env for agents — document either way. Secrets stay on
the host (shell / agenix); Nix may set non-secret URL hints only if useful.

## Tasks

- [ ] Add pinned `coolify-cli` derivation; expose as flake package
- [ ] Add `cda.aitools.coolify.*` options (off by default) and package wiring
- [ ] Document bootstrap: env vars and/or `coolify context add|set-token`
- [ ] Prove `nix build .#coolify-cli` and run kit verify

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Release asset names vary by arch | Use `stdenv.hostPlatform` / system map like other fetchurl CLIs |
| CLI is config-file based, not env | Document `context` + optional wrapper; do not invent fake env if unsupported |
| Hash drift on pin bump | Pin version + sha256; note bump procedure in README |

## Test plan

- [ ] `nix build .#coolify-cli` succeeds; `./result/bin/coolify version` works
- [ ] Module option compiles (nix eval / existing module pattern)
- [ ] `bash scripts/verify.sh` (or `mise run verify`) passes

## Verification

- [ ] Lint / test / build per project conventions
- [ ] No ADR needed unless packaging choice conflicts with ADR-001/003 (should not)
