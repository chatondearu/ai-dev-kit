# ADR-011 — cdamux unified state model

- **Status**: proposed
- **Date**: 2026-11-11
- **Tags**: #architecture #tooling #autonomy #resilience

## Context

Explore map [#228](https://github.com/chatondearu/ai-dev-kit/issues/228) unifies cdamux
sessions, sub-agents, `orch.sh` workers, run graphs, statuses and orchestrator↔worker
messages, so that an orchestrator session (any client) can drive interactive workers
visible in Fleet. The state-store inventory
([#229](https://github.com/chatondearu/ai-dev-kit/issues/229),
[findings](https://github.com/chatondearu/ai-dev-kit/blob/research/state-stores-inventory/research/state-stores-inventory.md))
found 27 stores; a worker's state is duplicated in 6 places, a run id in 4, agent
identity in 5 with two vocabularies (`cursor` vs `cursor-agent`). The session registry
follows a tmux rename but the sub-agent ledger does not.

This ADR grows one section per resolved decision ticket of the map; it becomes
`accepted` when the map hands off to the PRD.

## Decision

### 1. Entities and identifiers ([#231](https://github.com/chatondearu/ai-dev-kit/issues/231))

- **Agent** is the pivot entity: one client conversation (orchestrator, worker,
  sub-agent or free session). Fields include `client`, the client's resume id, model and
  tier, `role`, `parent` (agent id), `run`.
- **Host** is the tmux pane an agent runs in. It is an attribute of the Agent, and it is
  **mandatory for orchestrators and their workers** so the user can always open them. Only
  in-client sub-agents (e.g. Claude Code Task sub-agents) have no host.
- **Run** groups workers under one parent. An orchestrator owns one long-lived run; an
  `orch.sh` invocation it launches is a sub-run; a manual `orch.sh` invocation is a run
  without parent.
- **Node** is a step or task inside a run (today's graph nodes). **Message** is an inbox
  entry between orchestrator and workers.
- **Ids**: a short generated id per Agent (e.g. `a-7f3k2`), set at creation in
  `@cdamux_id` and in the agent's environment as `CDAMUX_AGENT_ID`. The tmux session name
  is only a label; renames never move state.
- **Clients** use product names: `claude`, `cursor`, `opencode`, `codex`. The binary
  (`agent`, `cursor-agent`, …) is resolved in one place at launch.
- **Resilience** comes from the Agent state file plus the client resume id, not from
  tmux: after a reboot, restore (ADR-009) rebuilds the host and resumes the conversation.
- `orch.sh` workers run in tmux by default (`ORCH_TMUX=1`); headless mode stays for tests
  and `--sandbox`.

### 2. State root, format and write path ([#232](https://github.com/chatondearu/ai-dev-kit/issues/232))

- **State root**: `CDAMUX_HOME`, default `${XDG_STATE_HOME:-~/.local/state}/cdamux`
  (today's `cdamux_state_dir`). Optional `CDAMUX_CONFIG` overrides the config file path,
  default unchanged (`${XDG_CONFIG_HOME:-~/.config}/cdamux/config.toml`).
- **Layout** (JSON / JSONL files, readable with `cat`/`jq`):

  ```
  $CDAMUX_HOME/
  ├── agents/<agent-id>.json    current Agent state, atomic write (tmp + rename)
  ├── runs/<run-id>.json        run + its nodes, with a `repo` field
  ├── inbox/<agent-id>.jsonl    messages received, append-only; <agent-id>.cursor = ack
  └── events.jsonl              optional append-only journal (debug, web)
  ```

- **Runs move out of the repo**: run, node and worker states live under
  `$CDAMUX_HOME/runs/`. `.orch/` in the repo keeps only work artefacts (logs,
  `worker-result.json`, worktrees). Fleet, the API and the orchestrator read one place.
- **Single write path**: hooks, `orch.sh` and workers write only through the CLI
  (`cdamux state set|append|get`). The file format stays private. Without `cdamux` on
  `PATH`, writers no-op silently (hooks stay fail-open).
- **Concurrency**: one writer per entity file; inbox appends are small (< 4 KB) single
  `O_APPEND` writes. No global lock.

### 3. Status set and precedence ([#233](https://github.com/chatondearu/ai-dev-kit/issues/233))

- **One status set for every Agent**:

  | Status | Reported by | Event |
  | ------ | ----------- | ----- |
  | `working` | client hook | prompt submit |
  | `idle` | client hook | turn end, session start |
  | `needs_input` | client hook | permission / notification (Cursor: pane heuristic, no event) |
  | `blocked` | worker CLI, `orch.sh` | waiting on the orchestrator or a dependency — not on the user |
  | `done` | worker CLI, `orch.sh` | `report --done`, PR opened |
  | `error` | worker CLI, `orch.sh` | failure, crash |
  | `dormant` | computed by readers | no live host, resumable (ADR-009) |

  All writes go through `cdamux state set` (§2).
- **Precedence**: the reported status wins. Pane heuristics (`window_activity`, prompt
  detection) apply only to an Agent with no hook signal, or for `needs_input` on clients
  without that event. An Agent `working` with no pane output for more than N minutes is
  shown `working ⚠` (possible hang or crash) without changing the stored status.
- **Stickiness**: `done` and `error` stay until the next prompt submit on that Agent.
- **Install**: the status hooks join `install.sh --cdamux-hooks` and
  `cda.aitools.cdamux.hooks.enable`, which stay opt-in (ADR-007).

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| tmux session as the pivot | Sub-agents and print-mode workers have no session; state breaks on rename; tmux does not survive a reboot anyway |
| tmux name as id | Breaks on rename (today's registry/ledger split) |
| Client resume id as id | Unknown before the first turn, not uniform across clients |
| Parent/child links only, no Run | `orch.sh` runs stay a separate concept with their own store |
| Binary names for clients | `agent` vs `cursor-agent` changes across Cursor versions |
| One root for config + state | Leaves the XDG split; forces a config migration |
| SQLite | Every hook needs `sqlite3` or a binary; files no longer readable directly |
| Event log only | Costly reads, retention to design; kept as an optional journal |
| Keep runs in the repo `.orch/` | Two places to read (repo + state root) |
| Public file format, direct writes | Format becomes a hard-to-change contract |
| Merge `blocked` into `needs_input` | Hides whether the user or another agent must act |
| Hooks always win, no heuristic | A crashed agent stays `working` forever |
| `done` lasts one turn | Finished work disappears before the user looks |
| Status hooks installed by default | Breaks the ADR-007 opt-in and the ADR-001 host-tool boundary |

## Consequences

- **Positive**: one identity per conversation across Fleet, graph, registry and inbox;
  renames and reboots stop losing state; every orchestrated worker can be opened.
- **Negative**: migration of the 27 stores (fog on map #228); `ORCH_TMUX=1` by default
  changes `orch.sh` behaviour for existing users.
- **Negative**: every writer depends on the `cdamux` binary (silent no-op without it);
  readers of `.orch/graph` and `/v1/orch-*` must move to the state root.
- **Negative**: statuses stay heuristic until the user enables `--cdamux-hooks`.
- **Constraint**: message transport and `orch.sh` integration are decided by tickets
  #234–#235 and appended here.

## References

- Map [#228](https://github.com/chatondearu/ai-dev-kit/issues/228); research
  [#229](https://github.com/chatondearu/ai-dev-kit/issues/229),
  [#230](https://github.com/chatondearu/ai-dev-kit/issues/230)
- [[ADR-005-cda-graph-orchestration|ADR-005]], [[ADR-007-cdamux-session-hierarchy|ADR-007]],
  [[ADR-009-cdamux-session-restore-registry|ADR-009]], [[ADR-010-worker-cli-inheritance-and-tiers|ADR-010]]
