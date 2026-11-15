# PLAN-364 — feat(cdamux): orch new --global outside any repo

- **Status**: ready
- **PRD**: [[../prd/PRD-357-global-orchestrator|PRD-357]]
- **GitHub**: [#364](https://github.com/chatondearu/ai-dev-kit/issues/364)
- **Date**: 2026-11-16

## Summary

`cdamux orch new --global` starts one orchestrator outside any repo, marked
`scope=global`, with a global boot prompt. A second call switches to the live one.
Fleet's spawn modal offers the same and shows the global orchestrator at top level.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] (proposed) | §1: `scope` field, at most one live global orchestrator, cwd outside any repo |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | State only via cdamux state API |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Restore keeps role + scope + boot behaviour |
| `cdamux/AGENTS.md` | Tests on a throwaway tmux server only |

## Scope

### In scope

- `cdamux/crates/cdamux-cli/src/main.rs` (`OrchCommands::New { global: bool }`, `cmd_orch_new`)
- `cdamux/crates/cdamux-core/src/orch.rs` (`OrchNewRequest.global`, scope in agent state,
  global boot prompt selection, single-live check)
- `cdamux/prompts/orch-boot-global.md` (new, ≤ 10 lines)
- `cdamux/crates/cdamux-cli/src/spawn_modal.rs` (global option on the orchestrator role)
- `cdamux/crates/cdamux-cli/src/fleet_tree.rs` (global orchestrator row at top level, its
  workers nest under it whatever their repo)
- Docs: cdamux CLI reference / `doc/orchestration.md` (short)

### Out of scope

- `worker spawn --repo` (#363), worker cap (#366), nested orchestrators (#365),
  `cda-orchestrator` § Global (#367)

## Technical approach

1. Cwd for `--global`: `$CDAMUX_HOME/global` (create it); `--path` with `--global` is an error.
2. Single live: scan agents with `role=orchestrator` and `scope=global` whose `host` is a
   live tmux session → print it and switch to it (inside tmux), exit 0, no new spawn.
3. Spawn as today (`spawn_orchestrator`, `CDA_ROLE=orchestrator`, prompt file) with the
   global boot prompt, then `state_append` `{"scope":"global"}`. Session name `c-global`
   (unique suffix if taken).
4. Global boot prompt: you are the global cdamux orchestrator; skill `cda-orchestrator`;
   list projects with `cdamux projects`; pick the repo per request (ask if ambiguous);
   dispatch with `cdamux worker spawn --repo` or `orch.sh --repo`; never work in a repo.
5. Fleet: an orchestrator with `scope=global` is a top-level row; spawn modal toggle
   `g` (or a field) "global" on the orchestrator role.

## Tasks

- [ ] CLI flag + request field + `$CDAMUX_HOME/global` cwd
- [ ] Single-live check (switch instead of spawn)
- [ ] `scope=global` in agent state; global boot prompt file
- [ ] Fleet top-level row + spawn modal option; golden snapshots updated
- [ ] Tests: throwaway tmux — spawn global, second call returns the same host; unit for
      prompt selection; snapshot for the Fleet row
- [ ] Docs
- [ ] `cargo test` in `cdamux/`; `bash scripts/verify.sh`; PR `Closes #364`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Overlap with #363 in `main.rs` | Different subcommands; rebase on main before PR |
| Restore re-sends the project boot prompt | Restore resumes the conversation; no boot re-send (check `restore.rs` keeps scope) |

## Test plan

- [ ] `cargo test` (incl. `CDAMUX_UPDATE_SNAPSHOTS=1` only for intended render changes)
- [ ] `bash scripts/verify.sh`
