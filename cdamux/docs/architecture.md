# cdamux — architecture (v0 target)

## Principles

1. **tmux is the database** — sessions, panes, and attach semantics stay in tmux.
2. **Daemon is optional but recommended** — classification, caching, tailnet API;
   CLI can degrade to tmux-only for `doctor` and early `list`.
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
             │         Unix socket / HTTP      │
      ┌──────▼───────────────────────────────▼──────┐
      │  cdamuxd                                     │
      │  - poll tmux                                 │
      │  - session state machine                     │
      │  - project discover (roots + mode)           │
      │  - optional: listen tailnet :PORT            │
      └──────┬──────────────────────────────────────┘
             │
      ┌──────▼──────┐
      │  tmux       │  default socket or `-L cdamux` (TBD)
      └─────────────┘
```

## Crates

| Crate | Responsibility |
| ----- | -------------- |
| `cdamux-core` | Config, types, `GroupBy`, discover, session id |
| `cdamuxd` | Daemon main loop, tmux parser, API handlers |
| `cdamux-cli` | clap, `list`/`attach`/`spawn`/`doctor`, `--pick` TUI |

## Config (`~/.config/cdamux/config.toml`)

See `config/cdamux.example.toml`. Highlights:

- `[[projects.roots]]` — path + optional label.
- `projects.discover` — `flat` | `depth(N)` | `git_only`.
- `sessions.group_by` — `project | cwd | agent | group_path | none`. Fleet tree uses
  discovered projects when `project`; other values regroup top-level nodes (TUI `g` cycles).
  Fleet omits projects/groups with **no tmux sessions** (full discover list stays on Projects tab).
- **Preview** — `tmux capture-pane -p -e -S -<n>` in the sidecar (snapshot, not a live PTY;
  no `-E -1`, which blanks panes with `history_size=0`). CLI renders ANSI via `ansi-to-tui`;
  **Enter** attaches for full interactive tmux. Session Active/Idle uses `pane_idle_time`;
  empty/unsupported → Idle (fail closed), not Active-via-zero.
- **Agent tag** — spawn sets `@cdamux_agent` (legacy `@ccmux_agent`); Fleet session rows
  and Preview chrome show that label, or `—` when unset.
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

## Nested tmux

When `$TMUX` is set (popup or inner session):

- Use `tmux switch-client -t <session>`.
- **`--pick`**: exit CLI with code 0 immediately after successful switch.
- Full dashboard TUI: stay running (future `cdamux` without `--pick`).

## API sketch (v0.2)

| Endpoint | Method | Purpose |
| -------- | ------ | ------- |
| `/v1/health` | GET | Daemon version, tmux reachable |
| `/v1/sessions` | GET | List with state + group fields |
| `/v1/sessions/:id/pane` | GET | Tail of pane (text) |
| `/v1/sessions/:id/send` | POST | Keys / line + optional enter |

Auth: tailnet only (bind to Tailscale IP); no public internet in v0.2.

Operator curl examples: [tailnet-api.md](tailnet-api.md).

## Relationship to ai-dev-kit

- Lives at `cdamux/` until extract (ADR-004).
- `aitools/` ships **cdamux** via `nix build .#cdamux` and `cda.aitools` modules.
- Orch worktrees stay `.orch/worktrees/` (no auto-bridge, ADR-003). cdamux spawn may
  create worktrees under `[worktrees].root` in `config.toml` only when the operator
  toggles worktree mode in the Fleet modal (`cdamux worktree list` inspects links).
- Fleet **reads** repo-local orch state: `.orch/graph/current.json` (Orch graph tab /
  `GET /v1/orch-graph?repo=`) and `.orch/run/status.json` (worker rows under a project).
  Execution stays in `orch.sh`; cdamux does not start workers.

## Open design choices

| Topic | Options | Default proposal |
| ----- | ------- | ------------------ |
| tmux socket | User default vs `-L cdamux` | User default v0 (see existing `c-*` sessions) |
| Status source | Hooks vs pane heuristics | Heuristics v0; hooks v0.1 for Cursor |
| TUI stack | ratatui vs skim in `--pick` | skim-style minimal v0, ratatui dashboard v0.1 |
