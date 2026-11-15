# PLAN-358 — fix(cdamux): deliver orchestrator boot prompt via prompt file and export CDA_ROLE

- **Status**: done
- **PRD**: [[../prd/PRD-356-orchestrator-actually-orchestrates|PRD-356]]
- **GitHub**: [#358](https://github.com/chatondearu/ai-dev-kit/issues/358)
- **Date**: 2026-11-15

## Summary

Orchestrators get their boot prompt on the client command line from
`$CDAMUX_HOME/prompts/<host>.md`, like workers, instead of a `send-keys` paste right after
spawn. Orchestrator panes export `CDA_ROLE=orchestrator` on spawn and respawn.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Role lives in agent state; env is only a signal for hooks |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Respawn from the registry record must keep the same behaviour |

## Scope

### In scope

- `cdamux/crates/cdamux-core/src/orch.rs` (`spawn_orchestrator`)
- `cdamux/crates/cdamux-core/src/worker.rs` (`write_first_prompt` → `pub(crate)`)
- `cdamux/crates/cdamux-core/src/spawn.rs` (`with_agent_id_env` call sites at spawn and respawn)
- Tests: `cdamux/crates/cdamux-core/tests/orch_tmux.rs`, unit tests in `spawn.rs` / `orch.rs`

### Out of scope

- Hooks (`cda-kiss`), skills, global orchestrator

## Technical approach

1. In `spawn_orchestrator`, compute `host` before building the launch command, write the
   boot prompt with `write_first_prompt(&host, orch_boot_prompt())`, pass the file to
   `with_first_prompt`. Keep paste-after for clients without CLI prompt support (and when
   no `CDAMUX_HOME` prompt file can be written).
2. Extend the env prefix (`with_agent_id_env`, used at `spawn.rs:154` and the respawn path
   `spawn.rs:184`) to add `CDA_ROLE=orchestrator` when the role is `orchestrator`
   (`SpawnExtra.role` on spawn, `SessionRecord.role` on respawn). Workers/agents: unchanged.
3. Replace the `orch_tmux.rs` assertion that relies on a pasted line reaching a stub shell
   with one that checks the launch command carries the prompt file (or the stub prints its argv).

## Tasks

- [x] Boot prompt via prompt file on the command line
- [x] `CDA_ROLE=orchestrator` prefix on spawn and respawn, not for workers
- [x] Tests: launch command has the file; env prefix present/absent by role
- [x] `cargo test` in `cdamux/`; `bash scripts/verify.sh`; PR `Closes #358`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| A client ignores positional prompt | Same client table as workers (`with_first_prompt`); paste fallback kept |
| Tests touch the live tmux server | Throwaway server only (`cdamux/AGENTS.md`) |

## Test plan

- [x] Unit: `with_agent_id_env`-style helper adds `CDA_ROLE` only for orchestrator
- [x] Integration (throwaway tmux): orchestrator launch command contains the prompt file path
- [x] `cargo test`, `bash scripts/verify.sh`
