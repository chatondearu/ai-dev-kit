# PLAN-290 — mise.toml + optional mise run verify (ADR-013)

- **Status**: ready
- **PRD**: [[../prd/PRD-290-mise-alongside-nix|PRD-290]]
- **GitHub**: [#290](https://github.com/chatondearu/ai-dev-kit/issues/290)
- **Date**: 2026-11-13

## Summary

Add optional Mise pins for Node and Rust (aligned with flake `devShell`),
document activate (no shims), and wire verify to prefer host → Mise →
`nix develop` fallback without removing Nix packages/modules/devShell.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-013-mise-alongside-nix-devshell|ADR-013]] | Stance B; activate / one outer mise exec; keep devShell; no shim-primary |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Host tools stay outside kit delivery |
| Skill `nix-develop-shell` | Never wrap git/gh; flake alone ≠ wrap everything |

## Scope

### In scope

- Root `mise.toml` + lock
- Verify entry / docs / AGENTS.md (+ skill note if needed)
- Pin Node ~24.x and Rust ~1.95 (match current `nix develop` toolchain)

### Out of scope

- Slim `devShells.default`
- aitools module rewrite
- install.sh / maid changes

## Technical approach

1. Declare `[tools]` node + rust in `mise.toml`; generate/commit `mise.lock`.
2. Prefer a `mise` task (e.g. `verify`) that runs `bash scripts/verify.sh` under
   the tool env, **or** document `mise exec -- bash scripts/verify.sh`.
3. Keep `scripts/verify.sh` itself Nix-agnostic (no hard dependency on mise);
   outer ladder lives in AGENTS.md / thin wrapper only if necessary.
4. Update `nix-develop-shell` one line if needed to mention activated Mise PATH.

## Tasks

- [x] Add `mise.toml` pinning Node and Rust to flake-shell-equivalent versions
- [x] Add `mise.lock` (or document why a backend cannot lock)
- [x] Document activate / `mise run`|`mise exec` verify path (no shims)
- [x] Update AGENTS.md verify oracle: host → Mise → `nix develop -c`
- [x] Ensure flake `devShell` / packages / modules untouched
- [x] Sanity: with Mise installed, `mise install` then verify without nix develop
      when possible; without Mise, Nix fallback still documented and runnable

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Mise rust ≠ nixpkgs rustc | Pin close to 1.95; document drift |
| Agents pick up shims | Docs + skill forbid shim-primary |
| verify.sh assumes nix | Keep script host-runnable; Mise only supplies PATH |

## Test plan

- [x] `mise install` succeeds from repo root (when mise present)
- [x] `bash scripts/verify.sh` under Mise env or host
- [x] Documented Nix fallback still valid
- [x] No accidental edits to `aitools/` modules or `install.sh`

## Verification

- [x] `bash scripts/verify.sh` or `nix develop -c bash scripts/verify.sh`
- [ ] PR `Closes #290`
