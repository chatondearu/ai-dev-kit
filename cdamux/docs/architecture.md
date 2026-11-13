# cdamux — architecture (v0 target)

## Principles

1. **tmux is the database** — sessions, panes, and attach semantics stay in tmux.
2. **Daemon = tailnet API server only** — `cdamux list` and the TUI read tmux
   directly (no IPC, no cache); `cdamuxd` is only needed for the tailnet HTTP API.
3. **Clients** — CLI first (`cdamux`, `cdamux --pick`); web on tailnet v0.2; mobile
   via Mosh/SSH to CLI or browser on tailnet (no native app v0).
4. **No telemetry** — default off, no network except explicit tailnet bind.

## Components

```text
┌─────────────────────────────────────────────────────────────┐
│  Terminals: Ghostty, phone (Mosh), browser (tailnet)        │
└────────────┬───────────────────────────────┬────────────────┘
             │                               │
      ┌──────▼──────┐                 ┌──────▼──────┐
      │  cdamux     │                 │  cdamux-web │
      │  (CLI/TUI)  │                 │  (static)   │
      └──────┬──────┘                 └──────┬──────┘
             │                               │ HTTP (tailnet)
             │                        ┌──────▼──────┐
             │                        │  cdamuxd    │
             │                        │  (API only) │
             │                        └──────┬──────┘
      ┌──────▼───────────────────────────────▼──────┐
      │  tmux  default socket or `-L cdamux` (TBD)   │
      └──────────────────────────────────────────────┘
```

## Crates

| Crate | Responsibility |
| ----- | -------------- |
| `cdamux-core` | Config, types, `GroupBy`, discover, session id |
| `cdamuxd` | Tailnet HTTP API server |
| `cdamux-cli` | clap, `list`/`attach`/`spawn`/`doctor`, `--pick` TUI |

## State root (`CDAMUX_HOME`)

Default `${XDG_STATE_HOME:-~/.local/state}/cdamux`. Override with `CDAMUX_HOME`.
Config stays on the XDG config path (`~/.config/cdamux/config.toml`) unless
`CDAMUX_CONFIG` points at a file. Layout (ADR-011):

```text
$CDAMUX_HOME/
├── agents/<agent-id>.json     # Agent entity (a-xxxxx); atomic write
├── agents/<session>.jsonl     # CLI sub-agent ledger (hooks; separate from entities)
├── runs/<run-id>.json         # Run + nodes
├── sessions/<session>.json    # Restore registry (ADR-009)
├── archive/agents|runs|inbox/ # Retention sink (`cdamux state gc`)
├── inbox/<agent-id>.jsonl     # messages received (append-only, ≤ 2 KB/line)
├── inbox/<agent-id>.cursor    # ack byte offset for `msg read --ack`
└── events.jsonl               # (optional journal)
```

Writers use `cdamux state set|get|append` and `cdamux msg send|read` (private JSON).
`cdamux state gc` archives then removes `done`/`error` agents and finished runs
(`ok`/`blocked`/`done`/`error`) with no live host whose file mtime is older than
`[state].retention_days` (default 14, must be ≥ 1) in `config.toml`. Aborts when
tmux cannot be probed (fail closed). An archived agent's `inbox/<id>.jsonl` and
`.cursor` move to `archive/inbox/`; an existing archive of the same name is kept and
the new one gets a `.<unix_secs>` suffix. Dormant resumable agents (session restore
registry, ADR-009) are never GC'd. Manual only (no timer in v0).
State files are written tmp + rename (`<file>.tmp.<pid>.<n>`). New agent ids skip any
id that already has `agents/<id>.json` or `inbox/<id>.jsonl`.
`orch.sh` / `graph-node.sh` publish run + nodes and one Agent per worker through
`cdamux state` only (hard cut #252; silent no-op without the binary). Repo
`.orch/` keeps work artefacts (locks, worker logs, progress) — not Fleet state.
With `$CDAMUX_AGENT_ID` set the run's `parent` is that agent (sub-run). Spawn
mints `a-xxxxx`, sets `@cdamux_id`, exports `CDAMUX_AGENT_ID`, and records the
product client name (`claude` | `cursor` | `opencode` | `codex`; aliases like
`agent` / `cursor-agent` normalise on write — orch `@cdamux_agent` uses the same
product names). Entity `parent` is the parent's agent id (resolved from the
registry), or `null` when unknown; `@cdamux_parent` stays the tmux session name
for Fleet nesting. Renaming a tmux session updates `host` via
`session_registry::rename` and does not move `agents/<id>.json`.
When orch is a sub-run, it also `cdamux msg send`s to the parent: one message per
worker reaching `done` / `blocked` / `error`, and one at run end with `ref` to
`.orch/integrator-report.md` (at-most-once markers under `.orch/run/`; no-op
without the binary or parent id).

### Inbox messages (ADR-011 §4)

`cdamux msg send --to <id|host|name> --kind report|ask|done|error|relay|answer [--ref path] "text"`
appends one JSON line to `$CDAMUX_HOME/inbox/<to>.jsonl` (`id`, `ts`, `from`, `to`,
`kind`, `text`, optional `ref`). `from` defaults to `$CDAMUX_AGENT_ID`. Target may be
an agent id, tmux host, or worker `name` (slice 2).

Delivery:

- **Worker** (`role=worker`): also pastes `text` into the worker pane (`send_to_session`).
- **Orchestrator / other**: append only; when status is `idle` and the pane has been
  quiet ≥ 10 s, types `📨 N messages — cdamux msg read` into the pane (no daemon).

`cdamux msg read [--agent id] [--ack] [--digest]` prints unread lines after the
ack cursor; `--ack` advances `inbox/<id>.cursor` to the end of the last complete line
read (a line still being written stays unread). Each line is appended with a single
`write(2)`, so concurrent senders do not interleave. `kind=ask` sets the sender
`blocked`; `kind=answer` clears the recipient to `idle`.

### Orchestrator session (`cdamux orch`)

- `cdamux orch new [--agent X] [--path DIR]` — tmux host with `role=orchestrator`, a
  fresh long-lived run under `$CDAMUX_HOME/runs/`, `$CDAMUX_AGENT_ID` exported, and a
  first-turn boot prompt that points at skill `cda-orchestrator` (kit
  `skills/core/cda-orchestrator/`, linked by `install.sh`). Fleet spawn modal role
  *orchestrator* (`o`) calls the same helper.
- Skill behaviours: spawn/track workers, relay (`@worker` forces; otherwise the
  orchestrator decides; one-line confirm), answer or escalate asks, never paste raw
  worker output, read reports by `ref`. See the skill file.

### Interactive workers (`cdamux worker`)

Requires `$CDAMUX_AGENT_ID` (caller must be an orchestrator session).

- `cdamux worker spawn <name> [--agent X] [--prompt-file f] [--here]` — tmux host with
  `role=worker`, `parent` = caller id, `run` = caller's long-lived run (minted if
  missing). Default: new git worktree `<root>/<run>/<name>` + branch
  `worker/<run>/<name>`, with `<root>` = `[worktrees].root` or `$CDAMUX_HOME/worktrees`
  (per-run, so two orchestrators can each spawn `api`). `--prompt-file` is read before
  the worktree is created. `--here` reuses the caller's cwd.
  Client defaults to the caller's product name unless `--agent` is set. Every spawn
  prefixes the first message with `cdamux/prompts/worker-preamble.md` (report via
  `cdamux msg`); `--prompt-file` content is appended. opencode's first message uses
  `--prompt`; other clients get a pane paste after launch.
- `cdamux worker ls` — workers whose `parent` is the caller.
- `cdamux worker peek <name> [--lines N]` — last N pane lines.

Opt-in hooks (`--cdamux-hooks`) inject an unread digest on turn-end (Claude `Stop`
`additionalContext`, Cursor `stop` `followup_message`) and on Claude
`UserPromptSubmit`. Cursor `beforeSubmitPrompt` cannot inject context — status
only; rely on `stop` + idle nudge. Details: [subagents.md](subagents.md).

## Config (`~/.config/cdamux/config.toml`)

See `config/cdamux.example.toml`. Highlights:

- `[[projects.roots]]` — path + optional label.
- `projects.discover` — `flat` | `depth(N)` | `git_only`.
- `sessions.group_by` — `project | cwd | agent | group_path | none`. Fleet tree uses
  discovered projects when `project`; other values regroup top-level nodes (TUI `G` cycles).
  Fleet omits projects/groups with **no tmux sessions** (full discover list stays on Projects tab).
- **Preview** — `tmux capture-pane -p -e -S -<n>` in the sidecar (snapshot, not a live PTY;
  no `-E -1`, which blanks panes with `history_size=0`). CLI renders ANSI via `ansi-to-tui`;
  **Enter** attaches for full interactive tmux. Session status prefers
  `$CDAMUX_HOME/agents/<id>.json` (`status` field, written by opt-in hooks via
  `cdamux state append`); transitional `@cdamux_state` is next; else pane heuristics
  from `now - #{window_activity}` (tmux has no `pane_idle_time`). Unknown idle → Idle
  (fail closed). Reported `idle`/`working` still run pane heuristics so Cursor
  (no permission event) can become `needs_input`. Reported `working` with a quiet
  pane past `daemon.working_warn_seconds` (default 600) shows `working ⚠` without
  rewriting state. `done` / `error` stick until the next prompt submit.
- **Agent tag** — spawn sets `@cdamux_agent` (legacy `@ccmux_agent`); Fleet session rows
  and Preview chrome show that label, or `—` when unset.
- **Session links** — spawn sets `@cdamux_role` (`agent`/`shell`/`orchestrator`/`worker`);
  fork also sets `@cdamux_parent` (source session). `@cdamux_run` carries an orch run id.
  All three come from the one `list-sessions -F` call. Fleet nests a session under its
  parent in the same project (any depth, `l`/`→` expands, `h`/`←` collapses); a dead parent
  or a parent cycle leaves it at the top of its project. `role=orchestrator` uses glyph
  `◎`. Unread inbox count shows as `✉N`; a worker with an unanswered `ask` shows `?`.
  Fleet composer Enter on a pending-ask row sends `cdamux msg` `kind=answer` (same on the
  tailnet web send form via `"kind":"answer"`). Non-session children (orch workers, CLI
  sub-agents) are `ChildRow`s with a `PreviewSource` (`Pane` / `FileTail` / `None`).
  Decision record: [ADR-007](../../foam/decisions/ADR-007-cdamux-session-hierarchy.md).
- **tmux workers** — with `ORCH_TMUX=1` (default), orch runs a worker's agent in a detached
  session `orch-<run>-<worker>` carrying the three options above (`@cdamux_role=worker`); the
  worker Agent under `$CDAMUX_HOME/agents/` gains `tmux_session` and `log`. A live session
  previews its pane, otherwise the Worker row tails the log (64 KiB bound). Without
  `ORCH_TMUX` Worker rows have no pane preview.
- **Steps** — each worker Agent has `phases` (`{phase, started_at}`);
  `cdamux-core::orch_steps` maps them to implement / verify / review / CI / PR with state and
  duration. Shown under the node detail in the Graph view and as a list in the Worker sidecar.
- **Agent** — a worker may also carry `cli`, `model`, `effort`, `tier` (ADR-010; all
  optional, older files parse). The Worker row ends with `[cli·tier]`; the sidecar leads with
  `cli·tier model effort`. `GET /v1/orch-status` passes them through. Read-only.
- **Refresh** — `refresh.rs` is the one scheduler: injectable-clock intervals, `$CDAMUX_HOME`
  runs/agents parsed when their mtime moves, session chrome fetched only for a session row,
  redraw only when something changed.
- `sessions.attach_mode` — `mirror | exclusive` (tmux `-d`), same semantics as skzv.
- `daemon.listen_tailnet`, `daemon.tailnet_port`.
- `[agents.cursor]` command override (kit: `cursor-agent`).
- `[notes]` — per-project markdown (local files only, ADR-004):
  - Default file: `<project>/.cdamux/notes.md`.
  - Optional `central_root` → one `{slug}.md` per project path under that directory.
  - CLI: `cdamux notes list|show|edit` (`show --render` uses **pulldown-cmark** for terminal output).
  - **Fleet sidecar**: the former Notes tab is **Foam** (ADR-012) — read-only list of
    `foam/**/*.md` anchored at git toplevel / discovered project root (not pane cwd).
    Missing or empty `foam/` shows an explicit empty state. CLI notes are unchanged;
    markdown preview and deep-link are follow-up issues.

## Session naming

v0: follow skzv convention optionally (`c-<project-basename>`) for interoperability
with existing sessions; document collision rules when same basename under different roots.

## Home session

Dashboard mode (`cdamux` with no subcommand) launched **outside** tmux runs
`tmux new-session -A -s cdamux cdamux`. That starts the server (so continuum can
restore when configured) and keeps a full-screen dashboard session that never
appears in Fleet / Sessions / `list_tmux_sessions`. When the home pane runs a
plain shell (tmux-resurrect / continuum restore it with the default shell over the
stale screen), `ensure_home_session` respawns it with `cdamux` — on outside-tmux
entry and before a kill. Killing the session the client is on switches to this home
session first. The kit template sets
`detach-on-destroy on` — not `next`/`previous`, which on tmux 3.7.x exit the
whole server on any `kill-session`. `kill_tmux_session` also flips those values
to `on` (and leaves them) when it sees them.

## Nested tmux

When `$TMUX` is set (popup or inner session):

- Use `tmux switch-client -t <session>`.
- **`--pick`**: exit CLI with code 0 immediately after successful switch.
- Full dashboard TUI: stay running (popup `M-p` unchanged; outside-tmux entry
  uses the home session above).
- Dashboard opens on the client's current session (project unfolded). Creating a
  session from the `n` modal switches the client to it; in a popup (no
  `TMUX_PANE`) the dashboard then exits so the popup closes.

## API sketch (v0.2)

| Endpoint | Method | Purpose |
| -------- | ------ | ------- |
| `/v1/health` | GET | Daemon version, tmux reachable |
| `/v1/sessions` | GET | List with state + group fields |
| `/v1/sessions/:id/pane` | GET | Tail of pane (text) |
| `/v1/sessions/:id/send` | POST | Keys / line + optional enter |

Auth: tailnet only (bind to Tailscale IP, unspecified addresses refused) plus optional
shared token `CDAMUX_TAILNET_TOKEN` / header `X-Cdamux-Token` (ADR-004 amendment 2026-11);
no public internet in v0.2.

Operator curl examples: [tailnet-api.md](tailnet-api.md).

## Relationship to ai-dev-kit

- Lives at `cdamux/` until extract (ADR-004).
- `aitools/` ships **cdamux** via `nix build .#cdamux` and `cda.aitools` modules.
- Orch worktrees stay `.orch/worktrees/` (no auto-bridge, ADR-003). cdamux spawn may
  create worktrees under `[worktrees].root` in `config.toml` only when the operator
  toggles worktree mode in the Fleet modal (`cdamux worktree list` inspects links).
- Fleet **reads** `$CDAMUX_HOME/runs/` and `$CDAMUX_HOME/agents/` (hard cut #252): one
  graph file per run, workers as Agents with matching `run`. Project rows and
  `GET /v1/orch-graph?repo=` take the newest live run (`&run=` picks one);
  `GET /v1/sessions/:id/orch-graph` prefers that session's own run, else its
  `@cdamux_run`. A run row (`ChildKind::Run`) sits under the session in its
  `session` field, headless runs under the project; the session sidecar and Graph
  view show the session's own newest run, else its `@cdamux_run`.
  The Orch graph tab and the full-screen Graph view (`g`) draw a layered DAG
  (`cdamux-core::orch_graph_layout`, longest-path layers over `edges` + `parent`)
  and fall back to the indented text tree when the layers do not fit the width.
  Execution stays in `orch.sh`; cdamux does not start workers.
- **Restore registry** ([ADR-009](../../foam/decisions/ADR-009-cdamux-session-restore-registry.md)):
  spawn and fork write `$CDAMUX_HOME/sessions/<session>.json`
  (`/` → `_`, atomic tmp + rename; home session and orch workers excluded). A snapshot
  for restore only, never read for liveness. Closing / renaming from cdamux removes /
  renames the file; an external kill keeps it. The stable agent id (`cdamux_id`) is
  not renamed with the session. `claude` spawns get `--session-id <uuid>`, recorded as
  `agent_session_id`. API in `cdamux_core::session_registry`: `record`, `load`,
  `load_all`, `remove`, `rename`, `set_agent_session_id`. Schema:

  ```json
  {
    "name": "c-proj",
    "cwd": "/home/me/dev/proj",
    "agent": "claude",
    "agent_session_id": "1772424a-d356-4b01-b206-647737b3def4",
    "cdamux_id": "a-7f3k2",
    "parent": null,
    "role": "agent",
    "layout": "single",
    "created_ts": 1793894041,
    "updated_ts": 1793894041
  }
  ```

  `agent`: product client or `shell`; `role`: `agent` | `shell` | `orchestrator` |
  `worker`; `layout`: `single` | `horizontal` | `vertical`; `agent_session_id`,
  `cdamux_id` and `parent` may be `null`; timestamps are Unix seconds.
- **Restore** (`cdamux restore <name>` / `--all`, `cdamux_core::restore`): recreates a
  recorded session that has no live tmux session of that name, through the spawn path
  (same name, cwd, layout, `@cdamux_agent` / `@cdamux_role` / `@cdamux_parent`), never
  switching the client. The primary pane runs the agent's `resume` template
  (`[agents.<id>].resume`, `{id}` = `agent_session_id`; defaults: claude
  `<command> --resume {id}`, cursor `<command> --resume={id}`, opencode
  `<command> --session {id}`, pi none). No id, an id with characters outside
  `[A-Za-z0-9._-]`, or no template → the plain agent command (fresh conversation; a fresh
  claude gets a new `--session-id`, recorded). Shell entries reopen a shell. A resumed
  record keeps its id and `created_ts`. Live sessions are skipped; a missing cwd is an
  error and the record is kept. `--all` restores parents before children. Restoring an
  **orchestrator** by name also restores its workers (registry `parent` host links and
  agent-state `parent` ids), parents first — same per-session status lines and exit 1 if
  any fail. One line per session (`restored`, `fresh (no id)`, `fresh (no resume command)`,
  `skipped (live)`, `error: …`). API: `restore_one`, `restore_targets`, `restore_subtree`,
  `resume_command`, `parents_first`, `dormant(records, &live_names)`.
- **Dormant rows** (Fleet): the dashboard caches the registry in `StatusCache`
  (`refresh_registry`: re-read only when the `sessions/` dir mtime moves, no tmux call
  per row). Records with no live tmux session (home excluded) render as dimmed `◌ name
  dormant · agent cwd` rows (`TreeRow::Dormant`) after the live rows of the project
  holding their cwd (longest path; non-`project` `group_by` and project-less cwds → the
  unassigned list); a project with only dormant records keeps its node. Waiting-only
  hides them. `Enter` restores the row (`restore_targets` — orch expands to workers),
  `x` twice forgets it (`session_registry::remove`); any other key cancels the pending
  forget.
- **Restart** (`cdamux restart <name>` / `--all` [`--force`], `restart_plan`): for each
  live recorded session, kill (`kill_tmux_session`, record kept) then `restore_one`.
  Skipped: dormant records (use `restore`), the home session, shell entries, and
  busy sessions — reported `working` in `agents/<id>.json`, or transitional
  `@cdamux_state` of `running` / `active` / `working` / `busy` — unless `--force`.
  Parents first, the current client's session last. Killing the session the
  CLI runs in would hang the CLI up, so that one is handed to a detached
  `nohup cdamux restart <name> --force` (own process group,
  `CDAMUX_RESTART_DETACHED=1`, output appended to `$CDAMUX_HOME/restart.log`);
  the client lands on the next session (`detach-on-destroy`) and is not switched back.
  One line per session (`restarted, <restore label>`, `skipped (<reason>)`,
  `restarting in the background (current session)`, `error: …`); exit 1 if any failed.
- CLI sub-agents come from the per-session ledger fed by hook adapters
  (sub-agent rows under a session): [subagents.md](subagents.md). Hooks are opt-in
  (`./install.sh --cdamux-hooks`, or Nix `cda.aitools.cdamux.hooks.enable`).

## Open design choices

| Topic | Options | Default proposal |
| ----- | ------- | ------------------ |
| tmux socket | User default vs `-L cdamux` | User default v0 (see existing `c-*` sessions) |
| Status source | Reported vs heuristics | **`$CDAMUX_HOME/agents/<id>.json`** wins; transitional **`@cdamux_state`** next; else pane heuristics. Pane `needs_input` still overrides reported `idle`/`working` (Cursor). Quiet reported `working` past `daemon.working_warn_seconds` → `working ⚠` (display only). Rust `may_overwrite` stickiness deferred to slice 2/3 (hooks use `ledger.js`) |
| TUI stack | ratatui vs skim in `--pick` | skim-style minimal v0, ratatui dashboard v0.1 |
