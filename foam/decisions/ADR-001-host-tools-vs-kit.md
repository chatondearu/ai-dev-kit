# ADR-001 — Host tools stay outside the kit

- **Status**: accepted
- **Date**: 2026-10-18
- **Tags**: #architecture #tooling

## Context

We want more autonomous agent runs (Claude / Cursor / opencode CLIs, optional
local models) without turning ai-dev-kit into a package manager for proprietary
or fast-moving host apps.

## Decision

1. The kit **links config** (`install.sh`, `nix/maid.nix`) — skills, rules, hooks.
2. Host apps stay on the machine (official installers or the user’s NixOS /
   home-manager / nix-darwin). The kit does not vendor or update them via
   `install.sh`.
3. `./scripts/doctor.sh` **checks** `PATH` and points at
   [`docs/recommended.md`](../../docs/recommended.md); it never installs.
4. Autonomous one-task runs use
   `skills/core/cda-dev/scripts/headless.sh` (agent + verify loop). Kanban /
   worktree orchestration remains `orch.sh`.
5. **Companion (opt-in):** an experimental **`aitools/`** Nix module
   (`cda.aitools`, see [[ADR-003-cda-aitools|ADR-003]]) may install
   Nix-friendly session tools (e.g. tmux) and wire versioned configs. It is
   **never** enabled by `install.sh`. Proprietary agent IDEs stay on official
   installers.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Install/update scripts for Claude, Cursor, OpenClaw in the kit | High churn; fights upstream installers; breaks portability |
| Ship host apps via the kit flake / `install.sh` | Nix packaging of Cursor/Claude is fragile; kit must stay usable without Nix |
| Vendor OpenClaw / ECC | Overlaps `cda-dev`; huge surface; keep as external only |
| Fold session apps into `install.sh` | Breaks portable kit-only path; rejected in favour of opt-in `aitools/` |

## Consequences

- **Positive**: Clear boundary; small maintenance; matches PHILOSOPHY portability.
- **Negative**: Users must install CLIs themselves (doctor tells them what is missing) unless they opt into `cda.aitools`.
- **Constraint**: Do not add package-install steps to `install.sh` for agent apps.

## References

- Operator docs: [`doc/orchestration.md`](../../doc/orchestration.md)
- `scripts/doctor.sh`, `skills/core/cda-dev/scripts/headless.sh`
- Companion: [[ADR-003-cda-aitools|ADR-003]], `aitools/`
- Personal notes: chatondearu foam `dev/openclaw-agents-autonomes`
