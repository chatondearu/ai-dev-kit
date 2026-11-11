# PLAN-241 — Add CDAMUX_HOME state CLI and stable agent ids

- **Status**: draft
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#241](https://github.com/chatondearu/ai-dev-kit/issues/241)
- **Date**: 2026-11-11

## Summary

Slice 1 foundation of PRD-239: one state root (`CDAMUX_HOME` / `CDAMUX_CONFIG`),
a single CLI write path (`cdamux state set|get|append`) for agents and runs, stable
agent ids that survive tmux renames, and normalised client product names.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §1–§2: entities, `CDAMUX_HOME` layout, atomic writes, private format, fail-open without binary |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Do not install host CLIs; resolve binaries at launch only |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Opt-in hooks stay out of this issue; hierarchy tags remain valid |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Registry/ledger paths follow `CDAMUX_HOME`; rename never moves state |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Work stays in-tree `cdamux/` until v0 |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `CDAMUX_HOME` default `${XDG_STATE_HOME:-~/.local/state}/cdamux`
- Optional `CDAMUX_CONFIG` for config.toml path
- `cdamux state set|get|append` for `agents/<id>.json` and `runs/<run-id>.json`
- Agent id `a-xxxxx` at creation → `@cdamux_id` + `CDAMUX_AGENT_ID`
- Client name normalisation: `claude` | `cursor` | `opencode` | `codex`
- Tests under a temp `CDAMUX_HOME`

### Out of scope

- Status set, hooks, Fleet status UI (#242)
- Messages / worker spawn / orch.sh hard cut (slices 2–3)
- Inbox / `events.jsonl` writers beyond creating empty layout if needed for dirs

## Technical approach

Extend cdamux core/CLI so the existing state directory becomes `CDAMUX_HOME`,
expose a small `state` subcommand as the only writer, mint stable agent ids at
session/agent creation, and map binary aliases to product client names in one
place used by spawn and registry.

## Tasks

- [ ] **State root**: `CDAMUX_HOME` / `CDAMUX_CONFIG` resolve with the ADR defaults; existing registry and ledger paths use the same root (acceptance: env override in a temp dir relocates all reads/writes)
- [ ] **CLI write path**: `cdamux state set|get|append` round-trips agent and run JSON with atomic tmp+rename for set; append is safe for the documented shapes (acceptance: CLI help + unit/integration tests on temp home)
- [ ] **Stable agent id**: creating an agent assigns `a-xxxxx`, sets `@cdamux_id`, exports `CDAMUX_AGENT_ID`; renaming the tmux session does not move or duplicate the agent file (acceptance: rename test)
- [ ] **Client names**: spawn/orch/registry accept aliases but persist only `claude|cursor|opencode|codex` (acceptance: table-driven normalisation tests)
- [ ] **Docs touch**: note `CDAMUX_HOME` / `state` in cdamux architecture or CLI help so operators can find the root
- [ ] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Dual write with old paths | Single resolver for the root; tests assert no writes outside `CDAMUX_HOME` when set |
| Breaking existing registry consumers | Keep subdirectory names (`sessions`, …) under the new root; migrate by path join only |
| Live state pollution in tests | Always set temp `CDAMUX_HOME` / `XDG_STATE_HOME` |

## Test plan

Seams: (1) env → path resolution, (2) `cdamux state` CLI I/O, (3) agent id + rename invariance, (4) client name normalisation.

- [ ] Temp `CDAMUX_HOME` unit/CLI tests for set/get/append and atomic replace
- [ ] Rename-does-not-move-state check
- [ ] Client alias → product name table
- [ ] `nix develop -c bash scripts/verify.sh`

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR only if a new decision was made
