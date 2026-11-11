# PRD-239 — Orchestrator session on the unified cdamux state model

- **Status**: approved
- **GitHub**: [#239](https://github.com/chatondearu/ai-dev-kit/issues/239)
- **Date**: 2026-11-11
- **Tags**: #product #orchestration #cdamux

## Problem

The maintainer wants what Orca's orchestrator gives: **one session** they keep talking
to, which spawns and drives the other sessions, relays their messages and gets their
reports, while they keep working in it. The kit cannot do this today:

- `orch.sh` workers run in print mode (`claude -p`, …): no message reaches them mid-run.
- State is spread over 27 stores (inventory, [#229](https://github.com/chatondearu/ai-dev-kit/issues/229));
  a worker's state lives in 6 places, agent identity in 5 under two names.
- Fleet statuses never update on interactive sessions: no hook writes `@cdamux_state`
  (the `pane_idle_time` bug was fixed in #220, the hook gap remains).
- Nothing carries a message from a worker back to the session that launched it.

Explore map [#228](https://github.com/chatondearu/ai-dev-kit/issues/228) settled the
model in [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]].

## Goals

- [ ] One source of truth under `CDAMUX_HOME` for agents, runs, statuses and messages,
      written only through the `cdamux` CLI.
- [ ] Real statuses in Fleet for every interactive session (Claude Code, Cursor CLI,
      opencode, codex) once `--cdamux-hooks` is on.
- [ ] Any client can be an orchestrator that spawns interactive workers (tmux +
      worktree), sends them messages and receives their reports, including while idle.
- [ ] `orch.sh` runs publish into the same model and report to a parent orchestrator.
- [ ] Fleet shows the orchestrator, its workers, unread messages and pending questions;
      everything survives a reboot through restore.

## Non-goals

- Orca decision gates and dispatch ids beyond `ask` / `answer` messages.
- MCP transport (ADR-011 §4: cannot start a turn; may wrap the CLI later).
- Native wake upgrades (Claude Code background `--wait`, opencode plugin) — optional,
  after v0.
- Installing client CLIs or enabling hooks by default
  ([[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]],
  [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]]).
- Double-writing the old `.orch/graph` / `status.json` files (hard cut, ADR-011 §5).
- Native mobile app, hosted VMs.

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer, any client | Opens Fleet → `n` → role *orchestrator* (or `cdamux orch new --agent cursor`) → chats with it |
| Orchestrator | "split this into API + UI" → `cdamux worker spawn api --prompt-file …` ×2 → two worktree workers appear under it in Fleet |
| Maintainer | Types "@ui use the Beamy design tokens" → orchestrator pastes it into the UI worker, confirms in one line |
| Worker | Blocked on a schema choice → `cdamux msg send --kind ask` → worker `blocked`; orchestrator answers or asks the maintainer (`needs_input`) |
| Idle orchestrator | Worker reports `done` → one-line nudge typed in the orchestrator pane → it reads and summarises |
| Maintainer | Opens any worker from Fleet to watch or type directly |
| Maintainer | Reboots the Mac → `cdamux restore` brings back the orchestrator and its workers on their conversations |
| Orchestrator | Launches `orch.sh --issue 12 --issue 15` as a sub-run → gets one message per worker `done`/`blocked`/`error` + run end |

## Requirements

Delivered in four slices; each one is usable alone. Child issues are created at intake.

### Slice 1 — State CLI and real statuses

- [ ] `CDAMUX_HOME` (default `${XDG_STATE_HOME:-~/.local/state}/cdamux`) and
      `CDAMUX_CONFIG` (default unchanged); existing registry/ledger paths follow.
- [ ] `cdamux state set|get|append` for `agents/<id>.json` (atomic tmp + rename) and
      `runs/<run-id>.json`; private format.
- [ ] Agent id generated at creation (`a-xxxxx`), stored in `@cdamux_id` and exported as
      `CDAMUX_AGENT_ID` to the agent; rename never moves state (fixes the
      registry/ledger split).
- [ ] Client names normalised to `claude`, `cursor`, `opencode`, `codex` across spawn,
      orch and registry.
- [ ] Status set `working` / `idle` / `needs_input` / `blocked` / `done` / `error` /
      `dormant`; reported status wins; heuristics only as fallback; `working ⚠` after N
      minutes without pane output (configurable); `done` / `error` sticky until the next
      prompt.
- [ ] Status hooks for the four clients (prompt submit → `working`, turn end → `idle`,
      permission/notification → `needs_input`) added to `install.sh --cdamux-hooks` and
      `cda.aitools.cdamux.hooks.enable`; fail-open, fast, no-op without `cdamux`.
- [ ] Fleet reads agent state from `CDAMUX_HOME`; `@ccmux_state` is no longer needed.

### Slice 2 — Messages, workers, orchestrator

- [ ] `cdamux msg send --to <id|name> --kind report|ask|done|error|relay|answer [--ref path] "text"`
      (≤ 2 KB) and `cdamux msg read [--ack]` on `inbox/<id>.jsonl` + `.cursor`.
- [ ] To a worker: paste into its pane (`send_to_session`) and log in its inbox.
- [ ] To an orchestrator: append; if it is `idle` and its pane is quiet for 10 s, type a
      one-line nudge (`📨 N messages — cdamux msg read`).
- [ ] Turn-end hook continues the orchestrator turn with the unread digest;
      prompt-submit hook adds it where the client supports it.
- [ ] `kind ask` sets the sender `blocked`; an `answer` clears it.
- [ ] `cdamux worker spawn <name> [--agent X] [--prompt-file f] [--here]`: worktree +
      branch by default, tmux host, `role=worker`, `parent` = caller's
      `CDAMUX_AGENT_ID`, run = the orchestrator's long-lived run; plus
      `cdamux worker ls|peek <name> [--lines N]`.
- [ ] `cdamux orch new [--agent X]` and an *orchestrator* role in the spawn modal start a
      client with the `cda-orchestrator` skill and `role=orchestrator`.
- [ ] Skill `cda-orchestrator`: spawn and track workers, relay rule (orchestrator decides,
      `@worker` forces, one-line confirmation), answer or escalate asks, never paste raw
      worker output, read reports by `ref`.
- [ ] Worker preamble (reuse `cda-dev/prompts/worker.md`): report blockers, questions
      and completion with `cdamux msg`.

### Slice 3 — `orch.sh` on the model

- [ ] `orch.sh` writes its run, nodes and one Agent per worker via `cdamux state`;
      sub-run of `CDAMUX_AGENT_ID` when set.
- [ ] Hard cut: stop writing `.orch/run/status.json` and `.orch/graph/*`; Fleet and
      `/v1/orch-graph|orch-status|orch-runs` read `CDAMUX_HOME/runs/`.
- [ ] `ORCH_TMUX=1` by default; headless only for tests and `--sandbox`.
- [ ] One `cdamux msg` to the parent per worker `done` (PR) / `blocked` / `error` and
      one at run end (`ref` = integrator report).
- [ ] `graph-node.sh` keeps its commands and calls `cdamux state`.
- [ ] Without `cdamux` on `PATH`: pipeline unchanged, state writes no-op.
- [ ] Absorbs the run-state leaks of [#236](https://github.com/chatondearu/ai-dev-kit/issues/236).

### Slice 4 — Fleet UI, restore, retention

- [ ] Fleet: orchestrator glyph, workers nested under it, unread badge `✉N`, `?` marker on
      a worker with a pending `ask`, reply to an ask from Fleet (composer → `answer`).
- [ ] Tailnet web shows the same tree, badges and asks.
- [ ] `cdamux restore` restores an orchestrator with its workers (hosts rebuilt, clients
      resumed by id, parent links kept).
- [ ] Retention: `done` / `error` agents and runs without a host for more than 14 days are
      archived then deleted (`config.toml` setting); `dormant` resumable agents are kept;
      `cdamux state gc` runs it by hand.

### Non-functional

- [ ] Hooks stay local-only, fail-open and fast (< 50 ms typical).
- [ ] Tests use temp `CDAMUX_HOME`, stub CLIs and a throwaway tmux server
      (`TMUX_TMPDIR` + `-L`); never the live config or tmux server.
- [ ] `nix develop -c bash scripts/verify.sh` green at every slice.
- [ ] Docs: `cdamux/docs/architecture.md`, `doc/graph-orchestration.md`,
      `doc/orca-mapping.md` (inbox now mirrored), `cda-graph` / `cda-dev` skills.

## Related project memory

- ADRs: [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] (the model),
  [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]],
  [[../foam/decisions/ADR-004-cdamux|ADR-004]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]],
  [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]],
  [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]],
  [[../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers|ADR-010]]
- Research: [state stores](https://github.com/chatondearu/ai-dev-kit/blob/research/state-stores-inventory/research/state-stores-inventory.md),
  [client capabilities](https://github.com/chatondearu/ai-dev-kit/blob/research/client-capabilities/research/client-capabilities.md)
- Related issues: #195 (session resume epic), #58 (mission control TUI), #236, #237

## Open questions

1. Cursor CLI hook firing is not run-verified (#230); slice 1 must confirm it on the
   maintainer host before relying on it.
2. opencode `--prompt` vs positional argument for the worker's first message: confirm when
   wiring `worker spawn` for opencode.
3. `working ⚠` threshold default (proposal: 10 min).

## Success metrics

- With `--cdamux-hooks` on, a Claude Code and a Cursor session show `working` while
  answering and `idle` after, in Fleet, within one refresh.
- From an orchestrator in Cursor CLI, two workers spawned, one `@worker` relay delivered,
  and a `done` report read without the user typing anything after the spawn.
- `rg -n 'status.json|graph/live' skills cdamux` finds no writer after slice 3.

## Implementation pointer

Plans per slice: `plans/PLAN-<child>-*.md`, created by `cda-plan` at intake.
