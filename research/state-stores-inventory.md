# State stores inventory (research #229, map #228)

Snapshot of `main` at `a08322d` (2026-11-10). Read-only survey of `cdamux/crates`,
`cdamux/hooks`, `skills/core/cda-dev/scripts`, `skills/core/cda-graph/scripts`, `aitools/`,
plus a read-only look at the maintainer's live tmux server and this repo's `.orch/`.

`$S` = `${XDG_STATE_HOME:-$HOME/.local/state}/cdamux` (Rust `cdamux_state_dir`,
`cdamux-core/src/subagent_ledger.rs:49`; JS `stateDir`, `cdamux/hooks/ledger.js:45`).
No dedicated env var overrides `$S` today: only `XDG_STATE_HOME`.

## 1. Stores

### Shared, per host

| # | Store | Path | Format | Writers (file:line) | Readers (file:line) | Lifetime / GC |
|---|-------|------|--------|---------------------|---------------------|---------------|
| 1 | `@cdamux_agent` | tmux session user option | string: config agent id (`claude`, `cursor`, `shell`) from spawn; **binary basename** (`claude`, `cursor-agent`, `agent`) from orch | `cdamux-core/src/spawn.rs:265` (via `link_options`, applied `:196`); `cda-dev/scripts/lib/agents.sh:271` | `cdamux-core/src/tmux.rs:102,131` (`list_panes_all`); `session.rs:126`; `session_chrome.rs:145` | tmux session life |
| 2 | `@cdamux_role` | tmux option | `agent` \| `shell` (spawn), `worker` (orch); `subagent` documented (`tmux.rs:34`) but never written | `spawn.rs:267`; `agents.sh:270` | `tmux.rs:41` (`list_tmux_sessions`); `session.rs:133` → Fleet | tmux session life; sessions spawned before the option existed have none |
| 3 | `@cdamux_parent` | tmux option | parent session name | `spawn.rs:269` (fork source); `agents.sh:267,273` (orch caller's `#S`) | `tmux.rs:41`; `session_links.rs:118` (Fleet nesting); `restore.rs:127` | tmux session life; mirrored in registry `parent` |
| 4 | `@cdamux_run` | tmux option | orch run id | `agents.sh:269` (ORCH_TMUX workers only) | `tmux.rs:41`; `cdamux-cli/src/fleet_tree.rs:90,96,295` | tmux session life |
| 5 | `@cdamux_state` | tmux option | `running` \| `waiting` \| `done` \| `error` (parser also takes `active`, `needs_input`, `idle`, `failed`, `blocked`) | `progress.sh:128` (phase mirror); `agents.sh:272` (`running`), `agents.sh:298` (rc → done/error, `ORCH_TMUX_KEEP=1`) | `tmux.rs:102,131`; `session.rs:68,85` (wins over pane heuristics); `restore.rs:146` (`restart` busy check) | tmux session life. **No hook sets it on interactive sessions** (`cdamux-hooks` wires SubagentStart/Stop + SessionStart only, `cdamux/hooks/cdamux-hooks:58-77`) |
| 6 | `@cdamux_return` | tmux option | `1` | `session_manage.rs:209` (on attach, `:220`) | `aitools/templates/tmux.conf:32` (`pane-died` hook reopens cdamux, then unsets) | Until the pane dies. Live server: set on all 18 sessions, i.e. effectively permanent |
| 7 | `@ccmux_*` (third party) | tmux option | `@ccmux_agent`; live server also has `@ccmux_state`, `@ccmux_prompts`, `@ccmux_seen` | skzv/ccmux (external) | only `@ccmux_agent`: `tmux.rs:102`, `session_chrome.rs:145` (fallback). `@ccmux_state` is **not** read | tmux session life |
| 8 | Sub-agent ledger | `$S/agents/<session>.jsonl` (`/` → `_`) | JSONL `{ts, cli, event: start\|stop, id, parent_id, name, transcript, status}` | `cdamux/hooks/ledger.js:58-66` via `claude-subagent.js:24`, `cursor-subagent.js:25`, `opencode-subagent.js:22,26` | `subagent_ledger.rs:118` (`session_subagents`, mtime-cached) ← `cdamux-cli/src/refresh.rs:165` | File trimmed to 500 lines (`ledger.js:47`); stopped rows shown 600 s (`subagent_ledger.rs:15`); **file never deleted**, not moved on session rename |
| 9 | Session restore registry | `$S/sessions/<session>.json` | JSON `SessionRecord {name, cwd, agent, agent_session_id, parent, role, layout, created_ts, updated_ts}` (tmp + rename) | `session_registry.rs:56` ← `spawn.rs:119,137,142`; `session_registry.rs:100` (rename) ← `cdamux-cli/src/tui.rs:601`; `session_registry.rs:92` (remove) ← `tui.rs:851,863`; `ledger.js:74-95` (`agent_session_id`) ← `session-start.js:16-17`, `opencode-subagent.js:19` | `refresh.rs:129` (Fleet dormant rows, `fleet_tree.rs:162`); `cdamux-cli/src/main.rs:631,634,672,675` (restore/restart); `session_manage.rs:137` (name collision) | Only explicit delete in the TUI; no GC. Dormant by design after `kill-session` |
| 10 | Restart log | `$S/restart.log` | text, append | `cdamux-cli/src/main.rs:727-732` | humans | Never rotated |
| 11 | cdamux config | `${XDG_CONFIG_HOME:-~/.config}/cdamux/config.toml` (`cdamux-core/src/lib.rs:374`) | TOML | TUI Settings `settings_form.rs:334` → `lib.rs:402`; `aitools/nix/maid-deploy.nix:58` (Nix-managed link); `aitools/scripts/cdamux-align-mac.sh:18` (copy if missing) | `lib.rs:381` ← `cdamux-cli/src/main.rs:282`, `tui.rs:161`, `cdamuxd/src/main.rs:14` | User-managed. `cdamuxd` reads it once at start |
| 12 | Hook wiring | `$CLAUDE_CONFIG_DIR/settings.json`, `$CURSOR_HOME/hooks.json`, `~/.config/opencode/plugins/cdamux-subagent.js`, link `~/.local/share/cdamux/hooks` | JSON / symlink | `cdamux/hooks/cdamux-hooks:55-122` ← `install.sh --cdamux-hooks`, `aitools/nix/cdamux-hooks-activation.nix:11` | the agent CLIs | Until `cdamux-hooks unwire` |
| 13 | Tailnet API | `cdamuxd` HTTP `/v1/sessions`, `/v1/orch-graph`, `/v1/orch-status`, `/v1/orch-runs`, `/v1/sessions/:id/{pane,orch-graph,send}` | JSON | none (stateless, re-reads tmux and files per request: `cdamux-core/src/tailnet_api.rs:335-519`) | web `cdamux/web/static/app.js:202-257` | **No server-side cache.** Only browser `localStorage.cdamux_token` (`app.js:21`) and env `CDAMUX_TAILNET_TOKEN` (`cdamuxd/src/tailnet_server.rs:31`) |
| 14 | In-process caches | Fleet `StatusCache` (`refresh.rs:88-290`), ledger cache (`subagent_ledger.rs:118`) | memory, mtime-gated | — | — | Process life |

### Per repo (`$REPO/.orch/`, main checkout via `lib/worktree.sh:5`)

| # | Store | Path | Format | Writers (file:line) | Readers (file:line) | Lifetime / GC |
|---|-------|------|--------|---------------------|---------------------|---------------|
| 15 | Live graph | `.orch/graph/live/<run_id>.json` (+ `.lock`) | JSON `{run_id, repo, pid, session, nodes[{id,kind,phase,status,message,parent,cli?,model?,effort?,tier?}], edges, updated_at, status, ended_at}` | `graph.sh:71-94` (init), `:166-208` (node), `:211-231` (edge), `:125-146` (finish) ← `orch.sh:507-508`, `graph.sh:243-466,589-757`, `headless.sh:108-121`, `cda-graph/scripts/graph-node.sh:83-104` | `orch_graph.rs:305` (`list_live_orch_runs`) ← `refresh.rs:217`; `tailnet_api.rs:364,422`; `graph-node.sh:62-67` (run by session) | Moved to `runs/` on finish; crashed runs archived at next `init` if pid dead and no write for 24 h (`graph.sh:60-69`). Fleet flags stale after 30 min (`orch_graph.rs:11`) |
| 16 | Archived runs | `.orch/graph/runs/<run_id>.json` | same | `graph.sh:43-57` | `orch_graph.rs:299` ← `refresh.rs:218`, `tailnet_api.rs:422` | Newest 20 kept (`graph.sh:54`) |
| 17 | `current.json` | `.orch/graph/current.json` | symlink → `live/…` or `runs/…` (legacy: a file) | `graph.sh:36-40` | `orch_graph.rs:73,83,113` ← `tailnet_api.rs:385` (default), `tailnet_api.rs:475` (`/v1/sessions/:id/orch-graph`), `orch_status.rs:87` | Always the latest run of the repo, whoever started it |
| 18 | Aggregate status | `.orch/run/status.json` | JSON `{updated_at, run_id, workers[{id, phase, message, updated_at, phases[], tmux_session?, log?, cli?, model?, effort?, tier?}]}` | `progress.sh:63-79` (`jq -s` over `*.json`) ← `orch_progress_set` (`orch.sh:211-410,517-575`, `graph.sh:255-343,605-725`, `ci.sh:61`), `orch_progress_agent` (`agents.sh:191`) | `orch_status.rs:73,80` ← `refresh.rs:263`, `fleet_tree.rs:178,208`; `tailnet_api.rs:400`; humans/agents polling (AGENTS.md) | Wiped at the start of each multi-issue `ready-pickup` (`orch.sh:505`); **not** wiped by `--graph-plan` (exits at `orch.sh:491` before the wipe) |
| 19 | Per-worker JSON | `.orch/run/<id>.json` | one worker object (same as above) | `progress.sh:47-62` | `progress.sh:40,45` (prev phases/agent); aggregated into #18 | Same as #18 |
| 20 | Per-worker line | `.orch/run/<id>.status` | text `PHASE\|message` | `progress.sh:94` | `progress.sh:108,152,175` (TTY dashboard); `orch.sh:582`; `graph.sh:437` (integrator fallback) | Same as #18 |
| 21 | Worker exit code | `.orch/run/<id>.rc` | text `0` \| `1` | `orch.sh:222,272,319,340,367,379,399,407,418`; `graph.sh:678,689,714` (graph-plan) | `orch.sh:429,548,567,574,583` | Same as #18 |
| 22 | Worker agent log | `.orch/run/<id>.log` | text | `agents.sh:284` (`tee`, ORCH_TMUX) | Fleet preview via `status.json.log` (`session_links.rs:68`) | Same as #18 |
| 23 | Misc run markers | `.orch/run/.dashboard-stop`, `.orch/run/.tier-warned-*` | empty files | `progress.sh:221`; `agents.sh:123` | `progress.sh:195`; `agents.sh:123` | Same as #18 |
| 24 | Agent rc handoff | `$TMPDIR/orch-agent-rc.XXXXXX` (not `.orch/run`) | text rc | runner, `agents.sh:285` | `agents.sh:290-294` | Deleted after read |
| 25 | Worker result | `<worktree>/.orch/worker-result.json` | JSON `{issue, pr_url, status: done\|blocked, checks: green\|red\|none, reason, files_touched, grounding}` (`templates/worker-result.schema.json`) | the agent, per `cda-dev/prompts/worker.md:24,74` | `graph.sh:151-161` (warn), `graph.sh:432-435` (integrator), `graph.sh:743-746` (plan integrator). **No cdamux reader** although `worker.md:74` says "cdamux reads it" | Lives in the worktree: removed with it on `done` (`orch.sh:426-433`), kept on `blocked` |
| 26 | Task rc / result | `<worktree>/.orch/task-<tid>.rc`, `task-<tid>-result.json` | text / JSON | `graph.sh:608,615,619`; the agent | `graph.sh:712,738` | Worktree life |
| 27 | Locks and AFK logs | `.orch/orch.lock`, `.orch/auto.lock`, `.orch/auto/*.log` | flock / text | `lib/lock.sh:18`; `orch-auto.sh:78-82` | same scripts | Logs: newest 50 (`orch-auto.sh:82`) |

Adjacent, out of #228 scope: `cda-kanban` keeps `.tmp/kanban/<owner>-<repo>.{status.json,cursor}` (`skills/core/cda-kanban/scripts/gh-board.sh:588-597`).

## 2. Duplicated facts

| Fact | Where it is stored |
|------|--------------------|
| **Run id** | graph filename + `.run_id` (#15), `status.json.run_id` (#18), `@cdamux_run` (#4), worker tmux session name `orch-<run>-<id>` (`agents.sh:239`) |
| **Which session owns a run** | graph `.session` (#15, creator's `#S`), `@cdamux_parent` on each worker session (#3, same caller), `@cdamux_run` (#4, reverse link) |
| **Worker state** (6 places) | `status.json` phase (#18), `<id>.json` (#19), `<id>.status` (#20), `<id>.rc` (#21), graph node `worker-<id>` status (#15), `@cdamux_state` (#5), plus the agent's own `worker-result.json.status` (#25) |
| **Agent identity** | `@cdamux_agent` (#1), registry `agent` (#9), `status.json` `cli/model/effort/tier` (#18), graph node `cli/model/effort/tier` (#15), ledger `cli` (#8) |
| **Parent / role** | `@cdamux_parent` + `@cdamux_role` (#2, #3) and registry `parent` + `role` (#9) |
| **Session existence** | tmux (truth), registry (#9, dormant included), `status.json.tmux_session` (#18, "may be gone") |
| **Conversation id** | registry `agent_session_id` (#9), ledger `parent_id` (#8, the parent's session id for Claude) |

## 3. Contradictions and gaps found

1. **pr-fix runs never finish their graph.** `orch.sh` exports `ORCH_RUN_ID` (`:97`); `pr-fix` reaches `orch_graph_upsert_node`, which lazily inits a live file (`graph.sh:174`), but no `orch_graph_finish` runs in the `pr-fix` branch (`orch.sh:606-718`). The file stays `running` in `live/` until a later `init` archives it 24 h on. Live evidence: `.orch/graph/live/20261110T135454Z.json` (nodes `worker-worker-*`, pid 60529 dead) shows as a live run in Fleet.
2. **Same phase, two states.** `pending` → `@cdamux_state=waiting` → NeedsInput (`progress.sh:125`, `session.rs:71`), but → Idle on the worker row (`session_links.rs:50`).
3. **`@cdamux_state` says `running` on a finished agent.** With `ORCH_TMUX_KEEP=1`, `agents.sh:298` sets done/error on agent exit, then the next `orch_progress_set` (verify, ci, review) rewrites `running` (`progress.sh:126`) on a dead pane.
4. **Graph worker node vs status.json.** The graph node `worker-<id>` turns `done` after verify (`graph.sh:306,319`) while `status.json` moves on to `ci`/`review` and may end `blocked` (`orch.sh:358-407`). The graph never shows the CI/review outcome.
5. **`status.json.run_id` can mislabel workers.** `--graph-plan` skips the `.orch/run` wipe (`orch.sh:471-491` vs `:505`), and `progress.sh:66-78` aggregates every `*.json` in the dir under the current `ORCH_RUN_ID`: workers left over from an earlier run appear under the new run.
6. **`current.json` ≠ the session's run.** `/v1/sessions/:id/orch-graph` and default `/v1/orch-graph` read `current.json` = the latest run of the repo (`tailnet_api.rs:385,475`), whereas Fleet picks runs by graph `.session` / `@cdamux_run` (`fleet_tree.rs:283-299`). Two sessions running orch in one repo: the web dashboard shows the wrong graph for one of them.
7. **Interactive sessions have no state store.** No hook writes `@cdamux_state` (live server: none of 18 sessions has it); state is pane heuristics only. Meanwhile third-party `@ccmux_state=needs_input` sits unread on 2 live sessions.
8. **Two vocabularies for agent ids.** `@cdamux_agent` is `cursor` from spawn but `cursor-agent`/`agent` from orch (`agents.sh:271` uses the binary basename).
9. **Role `subagent` exists only in docs** (`tmux.rs:34`); sub-agents live in the ledger, not in tmux.
10. **Session rename splits state.** TUI rename moves the registry record (`session_registry.rs:100`) but not the ledger file `agents/<old>.jsonl`; sub-agent rows of the renamed session vanish.
11. **Two staleness clocks for runs.** Fleet: 30 min (`orch_graph.rs:11`); harness archive: 24 h + dead pid (`graph.sh:62`). A crashed run is "stale but live" for ~23 h.
12. **Stale doc claims.** `worker.md:74` "cdamux reads it" (no reader in `cdamux/`); `graph.sh:4` "foam-graph read it" (no such reader in the tree).
13. **Config has two writers.** The TUI Settings save writes `config.toml` in place (`lib.rs:408`), while `cda.aitools` deploys it as a Nix-managed file (`maid-deploy.nix:58`): a save either fails or diverges from Nix.
14. **No GC** for ledger files, registry records of deleted-outside-cdamux sessions, `restart.log`, or `@cdamux_return` (set on every attached session, unset only by `pane-died`).
15. **No env var for the state root.** `$S` follows `XDG_STATE_HOME` only (#228 asks for an override). On the maintainer's Mac `$S` does not exist yet and the hooks are not wired in `/cda/.claude/settings.json`, so registry and ledger are empty there today.

## 4. Inputs for the ADR (#228)

- One run/worker record could replace #15 node `worker-<id>` + #18-#21 + `@cdamux_state` for orch workers; tmux options would become a projection (or only `@cdamux_run`/`@cdamux_parent` as links).
- `$S` is the natural root for the unified model (registry and ledger already live there); `.orch/` stays per-repo for worktree artifacts (#25, #26) and could hold only a pointer.
- The tailnet API is already stateless, so a single store changes only its readers (`/v1/orch-*`).
