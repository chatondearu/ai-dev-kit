# PLAN-188 — Graphify package + cda.aitools.graphify option

- **Status**: draft
- **PRD**: [[../prd/PRD-186-floorplan-graphify|PRD-186]]
- **GitHub**: [#188](https://github.com/chatondearu/ai-dev-kit/issues/188)
- **Date**: 2026-11-05

## Summary

Make the opt-in Nix companion install and configure Graphify (PyPI `graphifyy`,
Apache-2.0) behind `cda.aitools.graphify.enable`, exporting `CDA_GRAPHIFY=1`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | `install.sh` never installs graphify |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | Opt-in module, shared core + darwin/NixOS branches, relocatable root |

## Scope

### In scope

- Options: `graphify.enable`, `graphify.installMethod` (`auto | nix | uv | none`), `graphify.hooks.enable` (default false), `foamHome` (→ `CDA_FOAM_HOME`)
- `aitools/nix/graphify-package.nix` (pinned PyPI build); `auto` → `uv tool install graphifyy` fallback
- Session env `CDA_GRAPHIFY=1` on darwin + NixOS
- Idempotent `graphify install` (and `graphify claude install` when hooks on), honouring `CLAUDE_CONFIG_DIR`
- `aitools/prompts/bootstrap.md`, `aitools/README.md`, `scripts/doctor.sh`, `docs/recommended.md`
- `foam/decisions/ADR-008-graphify-code-graph.md` + index

### Out of scope

- Skill changes (#189), floorplan (#187), the user's nix-config

## Technical approach

Mirror the ccmux `installMethod` pattern in `common.nix` / `module-core.nix`;
activation step via the existing maid deploy path for `graphify install`.

## Tasks

- [ ] Options in `common.nix`, wiring in `module-core.nix`, darwin + nixos env
- [ ] Package derivation (or documented `uv` fallback if Nix build fails)
- [ ] Activation: `graphify install` idempotent; hooks sub-option
- [ ] doctor.sh check + test case
- [ ] Docs: bootstrap prompt, aitools README, recommended.md
- [ ] ADR-008

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| tree-sitter deps break the Nix build | `installMethod = auto` → uv |
| Activation writes into the live Claude config | honour `CLAUDE_CONFIG_DIR`; tests use temp HOME |

## Test plan

- [ ] Module eval with option on/off (darwin + nixos) in `nix flake check` or a scripted eval
- [ ] doctor output case

## Verification

- [ ] `nix develop -c bash scripts/verify.sh`
