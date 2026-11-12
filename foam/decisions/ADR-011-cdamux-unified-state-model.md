# ADR-011 — cdamux unified state model

- **Status**: accepted
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

Each section below records one resolved decision ticket of the map.

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

### 4. Orchestrator ↔ worker messages ([#234](https://github.com/chatondearu/ai-dev-kit/issues/234))

Client capabilities: [research #230](https://github.com/chatondearu/ai-dev-kit/blob/research/client-capabilities/research/client-capabilities.md).
Every client has a turn-end hook that can continue the turn; only Claude Code and
opencode can wake an idle session.

- **Message**: one JSON line ≤ 2 KB in `$CDAMUX_HOME/inbox/<agent-id>.jsonl` — `id`,
  `ts`, `from`, `to`, `kind` (`report` | `ask` | `done` | `error` | `relay` | `answer`),
  `text`, optional `ref` (path to `worker-result.json` or a log). Never raw output. Ack is
  the reader's `<agent-id>.cursor`. Written with `cdamux msg send`, read with
  `cdamux msg read [--ack]`.
- **Orchestrator → worker**: `cdamux msg send --to <worker>` pastes into the worker pane
  (`send_to_session`; Claude Code queues input mid-turn) and logs it in the worker inbox.
- **Worker → orchestrator**: append to the orchestrator inbox, delivered three ways:
  1. orchestrator busy → its turn-end hook continues the turn with the unread digest;
  2. orchestrator idle → the sender types a one-line nudge
     (`📨 N messages — cdamux msg read`) into its pane when its status is `idle` and the
     pane had no activity for 10 s. No daemon; works for every client;
  3. user writes → the prompt-submit hook adds the unread digest (clients that support it).
  Native wake (Claude Code background `cdamux msg read --wait`, opencode plugin) is an
  optional upgrade on the same inbox.
- **Worker questions**: `--kind ask` sets the worker `blocked`. The orchestrator answers
  when it can; otherwise it asks the user and goes `needs_input`.

### 5. `orch.sh` integration ([#235](https://github.com/chatondearu/ai-dev-kit/issues/235))

- **Publish through the model**: `orch.sh` writes its run, nodes and one Agent per
  worker with `cdamux state` (§1–3). With `CDAMUX_AGENT_ID` in the environment (launched
  by an orchestrator), the run is a sub-run of that Agent.
- **Hard cut** ([#252](https://github.com/chatondearu/ai-dev-kit/issues/252)):
  `orch.sh` stops writing `.orch/run/status.json` and
  `.orch/graph/live|runs|current.json`; Fleet and `/v1/orch-*` read
  `$CDAMUX_HOME/runs/`. Until that child lands, publish dual-writes both
  stores (amendment below) — not an open-ended second source of truth.
- **Repo artefacts stay**: `.orch/run/<id>.rc`, logs, `worker-result.json`,
  `integrator-report.md` and worktrees remain in the repo as work artefacts, not state.
- **Without `cdamux`**: the pipeline runs unchanged (worktrees, verify, PRs); state
  writes no-op, so there is no Fleet visibility. The kit keeps no dependency on cdamux
  (ADR-001).
- **Messages to the parent orchestrator**: one `cdamux msg` per worker reaching `done`
  (PR opened), `blocked` or `error`, and one at run end with `ref` to the integrator
  report.
- **`graph-node.sh`** stays as a thin wrapper: same `init` / `node` / `edge` / `finish`
  commands, calling `cdamux state`.

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
| Inbox + hooks for orchestrator → worker | An idle worker would not see the message without a nudge |
| Hooks only, never type into the orchestrator pane | An idle orchestrator sleeps until the user writes |
| `cdamuxd` watcher for the nudge | Requires the daemon to run |
| Every worker question goes to the user | The orchestrator exists to absorb what it can answer |
| MCP transport | Cannot start a turn on any client (#230); still needs the nudge |
| Double-write `.orch/` + state root indefinitely | Permanent second source of truth — rejected; a **bounded** dual-write until #252 is accepted (amendment 2026-11-12) |
| `orch.sh` file fallback without cdamux | Recreates a second source of truth |
| One message per run end only | The orchestrator misses a blocked worker until the run ends |
| Delete `graph-node.sh` | Breaks existing skills and prompts for no gain |

## Consequences

- **Positive**: one identity per conversation across Fleet, graph, registry and inbox;
  renames and reboots stop losing state; every orchestrated worker can be opened.
- **Negative**: migration of the 27 stores (fog on map #228); `ORCH_TMUX=1` by default
  changes `orch.sh` behaviour for existing users.
- **Negative**: every writer depends on the `cdamux` binary (silent no-op without it);
  readers of `.orch/graph` and `/v1/orch-*` must move to the state root.
- **Negative**: statuses stay heuristic until the user enables `--cdamux-hooks`.
- **Negative**: the idle nudge types into the orchestrator pane; a user typing after
  10 s of silence can collide with it.
- **Negative**: `orch.sh` runs are invisible in Fleet without `cdamux` on `PATH`; the hard
  cut rewrites the graph/status readers in one series of PRs.
- **Constraint**: the orchestrator v0 PRD builds on §1–5; Fleet UI, migration details,
  restore of an orchestrator with its workers and state GC are left to it (map #228 fog).

## Amendment 2026-11-12 — dual-write window until #252

Context: [PLAN-251](../../plans/PLAN-251-orch-cdamux-publish.md) / PRD-239 slice 3
ships publish via `cdamux state` ([#251](https://github.com/chatondearu/ai-dev-kit/issues/251))
before Fleet readers hard-cut
([#252](https://github.com/chatondearu/ai-dev-kit/issues/252)). A same-PR hard cut
would leave Fleet blind or force an oversized child.

Decisions (status stays **accepted**):

1. **Bounded dual-write**: from #251 until #252 merges, `orch.sh` /
   `graph-node.sh` write in-repo `.orch/graph` + `.orch/run/status.json` **and**
   `$CDAMUX_HOME` via `cdamux state`. Without `cdamux` on `PATH`, the state path
   no-ops; repo writers still run (ADR-001).
2. **Hard cut stays #252**: that child removes the `.orch` graph/status writers and
   points Fleet `/v1/orch-*` at `$CDAMUX_HOME/runs/`. No open-ended double-write.
3. **Sub-run unchanged**: with `$CDAMUX_AGENT_ID` set, the published run records
   that agent as `parent`.

## References

- Map [#228](https://github.com/chatondearu/ai-dev-kit/issues/228); research
  [#229](https://github.com/chatondearu/ai-dev-kit/issues/229),
  [#230](https://github.com/chatondearu/ai-dev-kit/issues/230)
- [[ADR-005-cda-graph-orchestration|ADR-005]], [[ADR-007-cdamux-session-hierarchy|ADR-007]],
  [[ADR-009-cdamux-session-restore-registry|ADR-009]], [[ADR-010-worker-cli-inheritance-and-tiers|ADR-010]]
