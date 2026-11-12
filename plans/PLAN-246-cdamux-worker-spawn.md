# PLAN-246 — Add cdamux worker spawn|ls|peek

- **Status**: done
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#246](https://github.com/chatondearu/ai-dev-kit/issues/246)
- **Blocked by**: #245
- **Date**: 2026-11-12

## Summary

Slice 2 workers: spawn interactive workers under the caller's orchestrator
(`worktree`+branch by default, tmux host, parent/run links), list them, and peek
pane output. Depends on message delivery from #245.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §1 host mandatory for workers; parent/run; product client names |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Hierarchy tags; workers visible in Fleet |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Registry + agent id at spawn; resume id when available |
| [[../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers|ADR-010]] | Default agent inherits caller unless `--agent` set |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Resolve binaries at launch; do not install CLIs |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `cdamux worker spawn <name> [--agent X] [--prompt-file f] [--here]`
- `cdamux worker ls` / `peek <name> [--lines N]`
- opencode first message via `--prompt`

### Out of scope

- `orch new` / skill / modal (#247)
- Message CLI itself (#245 — blocker)
- orch.sh publishing (slice 3)

## Technical approach

Build on existing spawn/worktree helpers and agent state from slice 1. Register
worker with `parent` = caller's `CDAMUX_AGENT_ID` and the orchestrator's run id.
Reuse `send_to_session` / peek patterns already used by review TUI.

## Tasks

- [x] **spawn**: create worktree+branch (unless `--here`), tmux host, agent id, `role=worker`, parent/run links, optional prompt file as first message (acceptance: integration test on throwaway tmux + temp home)
- [x] **opencode argv**: first message uses `--prompt` not positional (acceptance: unit test of command construction)
- [x] **ls / peek**: list workers under current orch (or by parent); peek last N pane lines (acceptance: CLI tests)
- [x] **msg ready**: spawned worker can be targeted by `cdamux msg send --to <name>` (acceptance: smoke with #245 APIs)
- [x] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Spawn without `CDAMUX_AGENT_ID` | Clear error: must run inside an orchestrator (or documented escape hatch) |
| Worktree collisions | Unique branch/name slug; fail loud if exists |

## Test plan

Seams: (1) spawn command construction, (2) agent state parent/run, (3) ls/peek, (4) msg targeting.

- [x] Throwaway tmux + temp `CDAMUX_HOME`
- [x] opencode `--prompt` argv table
- [x] `nix develop -c bash scripts/verify.sh`

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made
