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

## Config (`~/.config/cdamux/config.toml`)

See `config/cdamux.example.toml`. Highlights:

- `[[projects.roots]]` — path + optional label.
- `projects.discover` — `flat` | `depth(N)` | `git_only`.
- `sessions.group_by` — `project | cwd | agent | group_path | none`. Fleet tree uses
  discovered projects when `project`; other values regroup top-level nodes (TUI `G` cycles).
  Fleet omits projects/groups with **no tmux sessions** (full discover list stays on Projects tab).
- **Preview** — `tmux capture-pane -p -e -S -<n>` in the sidecar (snapshot, not a live PTY;
  no `-E -1`, which blanks panes with `history_size=0`). CLI renders ANSI via `ansi-to-tui`;
  **Enter** attaches for full interactive tmux. Session Active/Idle uses `now - #{window_activity}`
  (tmux has no `pane_idle_time`); unknown → Idle (fail closed), not Active-via-zero.
- **Agent tag** — spawn sets `@cdamux_agent` (legacy `@ccmux_agent`); Fleet session rows
  and Preview chrome show that label, or `—` when unset.
- **Session links** — spawn sets `@cdamux_role` (`agent`/`shell`); fork also sets
  `@cdamux_parent` (source session). `@cdamux_run` carries an orch run id. All three come
  from the one `list-sessions -F` call. Fleet nests a session under its parent in the same
  project (any depth, `l`/`→` expands, `h`/`←` collapses); a dead parent or a parent cycle leaves it at
  the top of its project. Non-session children (orch workers, CLI sub-agents) are
  `ChildRow`s with a `PreviewSource` (`Pane` / `FileTail` / `None`). Decision record:
  [ADR-007](../../foam/decisions/ADR-007-cdamux-session-hierarchy.md).
- **tmux workers** — with `ORCH_TMUX=1`, orch runs a worker's agent in a detached session
  `orch-<run>-<worker>` carrying the three options above (`@cdamux_role=worker`); the
  `status.json` entry gains `tmux_session` and `log`. A live session previews its pane,
  otherwise the Worker row tails the log (64 KiB bound). Without `ORCH_TMUX` nothing changes:
  Worker rows have no preview.
- **Steps** — each worker entry of `status.json` has `phases` (`{phase, started_at}`);
  `cdamux-core::orch_steps` maps them to implement / verify / review / CI / PR with state and
  duration. Shown under the node detail in the Graph view and as a list in the Worker sidecar.
- **Agent** — a worker entry may also carry `cli`, `model`, `effort`, `tier` (ADR-010; all
  optional, older files parse). The Worker row ends with `[cli·tier]`; the sidecar leads with
  `cli·tier model effort`. `GET /v1/orch-status` passes them through. Read-only.
- **Refresh** — `refresh.rs` is the one scheduler: injectable-clock intervals, `status.json`
  parsed only when its mtime moves, session chrome fetched only for a session row, redraw
  only when something changed.
- `sessions.attach_mode` — `mirror | exclusive` (tmux `-d`), same semantics as skzv.
- `daemon.listen_tailnet`, `daemon.tailnet_port`.
- `[agents.cursor]` command override (kit: `cursor-agent`).
- `[notes]` — per-project markdown (local files only, ADR-004):
  - Default file: `<project>/.cdamux/notes.md`.
  - Optional `central_root` → one `{slug}.md` per project path under that directory.
  - CLI: `cdamux notes list|show|edit` (`show --render` uses **pulldown-cmark** for terminal output).

## Session naming

v0: follow skzv convention optionally (`c-<project-basename>`) for interoperability
with existing sessions; document collision rules when same basename under different roots.

## Home session

Dashboard mode (`cdamux` with no subcommand) launched **outside** tmux runs
`tmux new-session -A -s cdamux cdamux`. That starts the server (so continuum can
restore when configured) and keeps a full-screen dashboard session that never
appears in Fleet / Sessions / `list_tmux_sessions`. Killing the last real session
the client is attached to ensures this home session exists first; the kit
template sets `detach-on-destroy next` (tmux ≥ 3.4) so the client lands there
instead of being kicked out of tmux.

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
- Fleet **reads** repo-local orch state: one graph per run under `.orch/graph/live/`
  (listed through the refresh scheduler, mtime-gated on `live/`, `runs/` and
  `current.json`; a running pre-#178 `current.json` stands in when `live/` is missing),
  `current.json` for the latest run (project rows, `GET /v1/orch-graph?repo=`, `&run=`
  picks one) and `.orch/run/status.json` (workers, nested under the run with the same
  `run_id`). A run row (`ChildKind::Run`) sits under the session in its `session`
  field, headless runs under the project; the session sidecar and Graph view show the
  session's own newest run, else its `@cdamux_run`.
  The Orch graph tab and the full-screen Graph view (`g`) draw a layered DAG
  (`cdamux-core::orch_graph_layout`, longest-path layers over `edges` + `parent`)
  and fall back to the indented text tree when the layers do not fit the width.
  Execution stays in `orch.sh`; cdamux does not start workers.
- **Restore registry** ([ADR-009](../../foam/decisions/ADR-009-cdamux-session-restore-registry.md)):
  spawn and fork write `${XDG_STATE_HOME:-~/.local/state}/cdamux/sessions/<session>.json`
  (`/` → `_`, atomic tmp + rename; home session and orch workers excluded). A snapshot
  for restore only, never read for liveness. Closing / renaming from cdamux removes /
  renames the file; an external kill keeps it. `claude` spawns get
  `--session-id <uuid>`, recorded as `agent_session_id`. API in
  `cdamux_core::session_registry`: `record`, `load`, `load_all`, `remove`, `rename`,
  `set_agent_session_id`. Schema (all keys always written):

  ```json
  {
    "name": "c-proj",
    "cwd": "/home/me/dev/proj",
    "agent": "claude",
    "agent_session_id": "1772424a-d356-4b01-b206-647737b3def4",
    "parent": null,
    "role": "agent",
    "layout": "single",
    "created_ts": 1793894041,
    "updated_ts": 1793894041
  }
  ```

  `agent`: config agent id or `shell`; `role`: `agent` | `shell`; `layout`:
  `single` | `horizontal` | `vertical`; `agent_session_id` and `parent` may be `null`;
  timestamps are Unix seconds.
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
  error and the record is kept. `--all` restores parents before children. One line per
  session (`restored`, `fresh (no id)`, `fresh (no resume command)`, `skipped (live)`,
  `error: …`); exit 1 if any failed. API: `restore_one(&SessionRecord, &AgentsConfig)
  -> Result<RestoreOutcome, RestoreError>`, `resume_command`, `parents_first`,
  `dormant(records, &live_names)` (registry minus live tmux names, parents first).
- **Dormant rows** (Fleet): the dashboard caches the registry in `StatusCache`
  (`refresh_registry`: re-read only when the `sessions/` dir mtime moves, no tmux call
  per row). Records with no live tmux session (home excluded) render as dimmed `◌ name
  dormant · agent cwd` rows (`TreeRow::Dormant`) after the live rows of the project
  holding their cwd (longest path; non-`project` `group_by` and project-less cwds → the
  unassigned list); a project with only dormant records keeps its node. Waiting-only
  hides them. `Enter` restores the row (`restore_one`), `x` twice forgets it
  (`session_registry::remove`); any other key cancels the pending forget.
- **Restart** (`cdamux restart <name>` / `--all` [`--force`], `restart_plan`): for each
  live recorded session, kill (`kill_tmux_session`, record kept) then `restore_one`.
  Skipped: dormant records (use `restore`), the home session, shell entries, and
  sessions whose `@cdamux_state` is `running` / `active` / `working` / `busy` unless
  `--force`. Parents first, the current client's session last. Killing the session the
  CLI runs in would hang the CLI up, so that one is handed to a detached
  `nohup cdamux restart <name> --force` (own process group,
  `CDAMUX_RESTART_DETACHED=1`, output appended to `${XDG_STATE_HOME}/cdamux/restart.log`);
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
| Status source | Hooks vs pane heuristics | Heuristics default; **`@cdamux_state`** overrides when set (`running`/`waiting`/`done`/`error`/`idle`) |
| TUI stack | ratatui vs skim in `--pick` | skim-style minimal v0, ratatui dashboard v0.1 |
