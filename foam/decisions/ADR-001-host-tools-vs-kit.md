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
   home-manager). The kit does not vendor or update them.
3. `./scripts/doctor.sh` **checks** `PATH` and points at
   [`docs/recommended.md`](../../docs/recommended.md); it never installs.
4. Autonomous one-task runs use
   `skills/core/cda-dev/scripts/headless.sh` (agent + verify loop). Kanban /
   worktree orchestration remains `orch.sh`.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Install/update scripts for Claude, Cursor, OpenClaw in the kit | High churn; fights upstream installers; breaks portability |
| Ship host apps via the kit flake | Nix packaging of Cursor/Claude is fragile; kit must stay usable without Nix |
| Vendor OpenClaw / ECC | Overlaps `cda-dev`; huge surface; keep as external only |

## Consequences

- **Positive**: Clear boundary; small maintenance; matches PHILOSOPHY portability.
- **Negative**: Users must install CLIs themselves (doctor tells them what is missing).
- **Constraint**: Do not add package-install steps to `install.sh` for agent apps.

## References

- Operator docs: [`doc/orchestration.md`](../../doc/orchestration.md)
- `scripts/doctor.sh`, `skills/core/cda-dev/scripts/headless.sh`
- Personal notes: chatondearu foam `dev/openclaw-agents-autonomes`
