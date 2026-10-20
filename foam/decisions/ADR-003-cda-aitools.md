# ADR-003 — cda.aitools companion (experimental)

- **Status**: accepted
- **Date**: 2026-10-20
- **Tags**: #architecture #tooling #deployment

## Context

Grill accord 2026-10-20: operators want versioned multi-machine setup for agent
terminal sessions (tmux / ccmux) without teaching `install.sh` to install apps
([[ADR-001-host-tools-vs-kit|ADR-001]]). Config roots are often not under `~/`
(e.g. `/cda/...` on the maintainer’s nix-darwin host).

## Decision

1. **Companion tree** `aitools/` in this monorepo (not a sibling repo yet).
2. **Opt-in Nix modules**: `nixosModules.cdaAitools` and `darwinModules.cdaAitools`
   share a common options/config core; OS-specific package backends may differ.
3. **Relocatable root**: `CDA_AITOOLS_HOME` (env) and module `configRoot` — single
   root for configs + light state in v1. Defaults are XDG-ish; `/cda/...` is a
   first-class example.
4. **Packages**: Nix-first for tmux; ccmux via `installMethod = auto | nix | brew | none`
   (Darwin auto may fall back to brew; NixOS does not require brew). Cursor/Claude
   CLIs stay on official installers.
5. **Bootstrap other machines**: `aitools/prompts/bootstrap.md` (+ optional wizard) —
   **not** a flattened project `cda-*` skill.
6. **Orch worktrees** stay kit-owned (`.orch/worktrees/`); cleanup is a cda-dev
   command. No auto-bridge to ccmux worktrees in v1.
7. **Status**: experimental until dogfood on ≥2 hosts (darwin first).

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Name the tree `host/` | Confusing vs ADR-001 “host tools”; use `aitools/` |
| Install prompt as default project skill | Anti-pattern in skills flatten; keep under `aitools/prompts/` |
| One `.nix` with OS hacks | Prefer shared core + darwin/linux branches |
| Stable README primary path immediately | Premature before dogfood |

## Consequences

- **Positive**: Kit-only path preserved; Nix users get a declarative companion.
- **Negative**: Two entry points to document; ccmux Nix packaging may be fragile.
- **Constraint**: Never enable companion packages from `install.sh`.

## References

- PRD: [`prd/PRD-33-cda-aitools.md`](../../prd/PRD-33-cda-aitools.md)
- [[ADR-001-host-tools-vs-kit|ADR-001]]
- `aitools/README.md`
