# ADR-009 — cdamux session restore registry

- **Status**: accepted
- **Date**: 2026-11-05
- **Tags**: #architecture #tooling #resilience

## Context

[[ADR-004-cdamux|ADR-004]] made tmux the database: cdamux keeps no store of its own.
That also means a session is gone for good when its tmux window is killed by mistake,
when the machine reboots, or when the operator restarts every agent to load an
updated kit. Epic [#195](https://github.com/chatondearu/ai-dev-kit/issues/195)
(PRD-195) wants those sessions back on the same agent conversation. tmux cannot hold
that memory: its user options die with the server.

Issue #196 called this ADR-008; that number was already taken (graphify).

## Decision

Amends ADR-004.

1. **tmux stays the truth for what runs.** The registry is a restore snapshot only:
   nothing reads it to decide whether a session is live, its state, or the Fleet tree.
   A recorded session with no tmux session of that name is *dormant*.
2. **One JSON file per session**:
   `${XDG_STATE_HOME:-$HOME/.local/state}/cdamux/sessions/<session>.json`, with `/` in
   the name mapped to `_` (same rule as the sub-agent ledger, ADR-007). Written atomically
   (tmp file + rename). Fields: `name`, `cwd`, `agent` (config agent id or `shell`),
   `agent_session_id` (nullable), `parent` (nullable), `role` (`agent` / `shell`),
   `layout` (`single` / `horizontal` / `vertical`), `created_ts`, `updated_ts` (Unix
   seconds). Schema: `cdamux/docs/architecture.md`; code:
   `cdamux-core/src/session_registry.rs`.
3. **What is recorded**: every agent or shell session cdamux spawns or forks
   (`spawn_tmux_session_with_layout` with an agent label). Not the home session, not
   orch workers (`orch.sh` spawns those, ADR-007).
4. **Claude pre-assign**: when the agent is `claude`, spawn appends
   `--session-id <uuid v4>` (16 bytes of `/dev/urandom`, no new crate) and records it,
   so a Claude session can be resumed without any hook. Other agents' ids come from
   opt-in `SessionStart` hooks (R2, #197).
5. **Lifecycle**: closing a session from cdamux removes its file; renaming it from cdamux
   renames the file. A session killed outside cdamux, or lost with the server, keeps its
   file (that is the restore case). New spawns skip a name that has a record
   (`c-proj-2`), so a dormant snapshot is never overwritten.
6. **Fail-open**: a registry write error never fails a spawn; the status line says the
   session was not recorded.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| tmux-resurrect / tmux-continuum | Restores panes and scrollback, not the agent conversation; another plugin to install; saves on a timer, not on spawn |
| tmux user options only | Lost with the tmux server (reboot, `kill-server`), the exact case to cover |
| One shared registry file | Needs locking between the TUI, CLI and hooks writing at once; a per-session file needs none (same reasoning as the ledger) |
| Read the registry to build the Fleet | Two sources of truth that drift; ADR-004 keeps tmux authoritative |

## Consequences

- **Positive**: a dead session can be restored on its conversation; survives reboots;
  hook writers (JS) and cdamux share a plain JSON file.
- **Negative**: files of sessions killed outside cdamux accumulate until restored or
  forgotten (R4 adds a forget key); a configured `claude` command that does not accept
  `--session-id` (a wrapper script) breaks at spawn.
- **Constraint**: tests that write the registry point `XDG_STATE_HOME` at a temp dir;
  tmux tests use a throwaway server (`cdamux/AGENTS.md`).

## References

- `cdamux/crates/cdamux-core/src/session_registry.rs`, `spawn.rs`, `subagent_ledger.rs`
- `cdamux/crates/cdamux-cli/src/tui.rs` (kill / rename)
- PRD: [`prd/PRD-195-cdamux-session-resume.md`](../../prd/PRD-195-cdamux-session-resume.md),
  plan: [`plans/PLAN-195-cdamux-session-resume.md`](../../plans/PLAN-195-cdamux-session-resume.md)
- Issues: [#195](https://github.com/chatondearu/ai-dev-kit/issues/195),
  [#196](https://github.com/chatondearu/ai-dev-kit/issues/196)
- [[ADR-004-cdamux|ADR-004]], [[ADR-007-cdamux-session-hierarchy|ADR-007]]
