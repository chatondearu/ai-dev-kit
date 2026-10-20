# PLAN-35 — aitools skeleton + nix modules

- **Status**: in-progress
- **PRD**: [[../prd/PRD-33-cda-aitools|PRD-33]]
- **GitHub**: [#35](https://github.com/chatondearu/ai-dev-kit/issues/35)
- **Date**: 2026-10-20

## Summary

Create `aitools/` tree and `cda.aitools` nixosModules + darwinModules with shared core and `CDA_AITOOLS_HOME` / configRoot.

## Tasks

- [ ] aitools/README.md experimental
- [ ] common.nix + darwin.nix + linux.nix (or module split)
- [ ] Wire flake outputs nixosModules.cdaAitools + darwinModules.cdaAitools
- [ ] Honor CDA_AITOOLS_HOME and configRoot (e.g. /cda/...)
- [ ] Document dogfood darwin-first

## Test plan

- [ ] `nix flake check` or build module eval on aarch64-darwin best-effort

## Verification

- [ ] Module options document installMethod placeholder for #36
