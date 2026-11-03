# ADR-007 — cdamux session hierarchy: tmux links, tmux workers, sub-agent ledger

- **Status**: accepted
- **Date**: 2026-11-03
- **Tags**: #architecture #tooling #autonomy

## Context

Epic [#155](https://github.com/chatondearu/ai-dev-kit/issues/155) (PRD-155,
PLAN-155) makes the cdamux Fleet show what is actually running: which agent
launched which, which orch worker runs where, and which sub-agents a CLI spawned
inside its own process. Before it, Fleet was a flat list of tmux sessions plus
worker rows read from `.orch/run/status.json`; an orch worker was a headless
child process (no pane to preview or attach to), and a CLI sub-agent
(Claude Code `Task`, Cursor sub-agent) was invisible because tmux cannot see
inside a CLI process.

[[ADR-004-cdamux|ADR-004]] fixed "tmux is the database": cdamux reads tmux, it
does not keep its own store. [[ADR-005-cda-graph-orchestration|ADR-005]] added
the read-only graph files (`current.json`, `status.json`). [[ADR-002-orch-auto-and-fast-lane|ADR-002]]
made autonomy opt-in. The hierarchy has to respect all three.

## Decision

1. **Links are tmux user options on the session**, read by the existing single
   `list-sessions -F` call (no per-session tmux call):
   - `@cdamux_parent`: name of the session that launched this one (fork, orch
     worker started from a cdamux session);
   - `@cdamux_run`: orch run id (`ORCH_RUN_ID`);
   - `@cdamux_role`: `agent`, `shell` or `worker`.

   Fleet nests a session under its parent in the same project node, at any depth
   (`session_forest` in `cdamux-core/src/session_links.rs`). A dead parent, a
   parent in another project node, or a parent cycle leaves the session at the top
   of its project. Spawn always sets `@cdamux_role`; fork also sets
   `@cdamux_parent`. Existing options keep their meaning (`@cdamux_agent`,
   `@cdamux_state`).
2. **`ORCH_TMUX=1` is an opt-in** (off by default, ADR-002): a worker's agent runs
   in a detached tmux session `orch-<run>-<worker>` (sanitized) with
   `@cdamux_run`, `@cdamux_role=worker`, `@cdamux_agent`, `@cdamux_state`, and
   `@cdamux_parent` when orch itself runs inside tmux. The session belongs to the
   worker id: the agent calls of one worker (implement, review fix, ...) reuse it
   by respawning its pane. Output is `tee`d to `.orch/run/<worker>.log` and echoed
   back on stdout, so verdict parsing is unchanged. The session is killed when the
   agent exits, unless `ORCH_TMUX_KEEP=1` (then `@cdamux_state` is `done` /
   `error`). `ORCH_RUN_ID` is exported by `orch.sh` only in this mode; Ctrl-C also
   kills the run's sessions. `progress.sh` mirrors the phase as `@cdamux_state`
   and adds `tmux_session` and `log` to `status.json` entries (only with
   `ORCH_TMUX=1`).
3. **The tmux server does not inherit orch's environment.** A runner script
   (`mktemp`, mode 0600) replays `export -p`, deletes itself first thing, and runs
   the agent. Credentials therefore never sit in a world-readable file or in the
   tmux command line.
4. **`ORCH_TMUX` is ignored under `--sandbox`** (a note is printed): the tmux
   server runs outside bubblewrap, so the agent would escape the sandbox. tmux
   missing or `new-session` failing falls back to the inline agent.
5. **Worker previews**: a worker row previews its pane while the session is live
   (`PreviewSource::Pane`, checked against the tree's session list) and
   otherwise the tail of its log (`FileTail`, bounded 64 KiB read). Rows that are
   not tmux sessions are `ChildRow { kind, state, preview }` leaves.
6. **Sub-agent ledger** (CLI sub-agents, `ChildKind::Subagent`): one JSONL file
   per tmux session, `${XDG_STATE_HOME:-$HOME/.local/state}/cdamux/agents/<tmux-session>.jsonl`
   (`/` in the name becomes `_`), one `start` / `stop` line per sub-agent with
   `ts`, `cli`, `event`, `id` and optional `parent_id`, `name`, `transcript`,
   `status`. Hooks append, trim to 500 lines; cdamux re-parses on mtime/length
   change, folds by `id`, keeps a stopped sub-agent 10 minutes, and previews the
   transcript tail (last 256 KiB, user/assistant text only). Schema and adapter
   rules live in `cdamux/docs/subagents.md`.
7. **Adapters** share one writer, `cdamux/hooks/ledger.js`; fail-open, local file
   only, no network. Install is explicit: `./install.sh --cdamux-hooks`
   (`--uninstall` removes); a plain `install.sh` neither adds nor removes them.

   | CLI | Adapter | Gap |
   | --- | ------- | --- |
   | Claude Code | `SubagentStart` / `SubagentStop` hooks | field names from the docs plus the on-disk transcript layout, not a recorded payload |
   | Cursor | `subagentStart` / `subagentStop` hooks | payloads from the docs, not recorded live; the docs do not say the CLI (`cursor-agent`) runs them |
   | opencode | plugin on `session.created` (with `parentID`) / `idle` / `error` / `deleted` | no transcript, so no preview; not run live; a resumed child is not shown again |
   | pi | none | pi has no sub-agents; separate pi processes are already tmux panes |
8. **UI surface that rides on this** (see `cdamux/docs/architecture.md`): the
   drawn orch DAG (Graph view on `g`, `group_by` cycling moved to `G`), the
   per-worker steps bar built from `phases` in `status.json`, and one refresh
   scheduler (`refresh.rs`: injectable-clock intervals, mtime-gated `status.json`
   cache) so no loop re-reads a file or calls tmux without need.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Store the hierarchy in a cdamux file or daemon | Breaks ADR-004 ("tmux is the database"); two sources of truth that drift when a session dies |
| Encode the parent in the session name | Rename-hostile and collides with the `c-<basename>` convention; options are invisible to users and free in `list-sessions -F` |
| `ORCH_TMUX` on by default | Changes how every worker runs and where its secrets travel; ADR-002 keeps autonomy features opt-in |
| Pass the environment through `tmux new-session -e` / the command line | Visible in `ps` and tmux history; the self-deleting 0600 runner is the smaller leak |
| Run tmux workers inside the sandbox | The tmux server is a separate process outside bwrap; sandboxing it is a different design |
| Watch process trees to find sub-agents | Needs per-CLI process knowledge and polling; each CLI's own hook is the one reliable signal |
| Ledger in a single shared file or in the repo | A per-session file bounds each file, needs no locking across sessions, and stays out of git |
| Wire hooks in plain `install.sh` | Hooks run on every sub-agent event of the user's live CLI; opt-in only, like the other experimental surfaces |

## Consequences

- **Positive**: Fleet shows the real tree (session -> fork -> worker -> sub-agent);
  an orch worker can be previewed live and attached to; sub-agent rows carry a
  transcript preview; everything degrades to the old flat view when the options,
  the log or the ledger are absent.
- **Negative**: with `ORCH_TMUX=1` each worker needs tmux and a 0600 temp file
  per agent call; renaming a parent session orphans its children (they become
  roots); a parent in another project node does not nest; two hooks trimming at
  the same instant can drop one ledger line.
- **Constraint**: tests that touch tmux use a throwaway server (`TMUX_TMPDIR` temp
  dir plus `-L <name>`); hook tests use a stub `tmux` on `PATH`; install tests
  override `HOME`, `CLAUDE_CONFIG_DIR`, `CURSOR_HOME`, `XDG_CONFIG_HOME`.
- **Not verified live**: Cursor CLI hooks, opencode events, and `@cdamux_parent`
  with a real `$TMUX` client are covered by unit tests only; revisit when a live
  run is possible.

## Amendments

- **2026-11-03** ([#178](https://github.com/chatondearu/ai-dev-kit/issues/178)):
  graphs are per run under `.orch/graph/live/<run_id>.json` (top-level `session`,
  the creator's tmux `#S` when inside tmux, and `pid`); `current.json` is a
  compatibility symlink to the latest run; `orch_graph_finish` moves the file to
  `runs/<run_id>.json`; live files with a dead pid and no write for 24 h are
  archived on the next init. `orch.sh` now exports `ORCH_RUN_ID` in every run, so
  the graph file name, `status.json` `run_id` and `@cdamux_run` share one id.
  `graph-node.sh` targets `--run-id`, else `$ORCH_RUN_ID`, else the newest live
  run of the current tmux session.
- **2026-11-03** ([#179](https://github.com/chatondearu/ai-dev-kit/issues/179)):
  cdamux reads the per-run files: a live run is a `ChildKind::Run` row under its
  `session` (headless runs under the project) holding the status.json workers of
  the same `run_id` and the worker sessions tagged `@cdamux_run`. Graph selection:
  run row → that run; session → its newest own live run, else its `@cdamux_run`;
  project → latest (`current.json`). Graph view `Tab` cycles a session's runs.

## References

- `cdamux/crates/cdamux-core/src/session_links.rs`, `tmux.rs`, `spawn.rs`,
  `subagent_ledger.rs`; `cdamux/crates/cdamux-cli/src/fleet_tree.rs`, `refresh.rs`
- `skills/core/cda-dev/scripts/lib/agents.sh`, `lib/progress.sh`, `orch.sh`;
  test: `skills/core/cda-dev/scripts/tests/test_tmux.sh`
- `cdamux/docs/subagents.md`, `cdamux/hooks/`, `./install.sh --cdamux-hooks`
- `doc/configuration.md` (`ORCH_TMUX`, `ORCH_TMUX_KEEP`), `doc/graph-orchestration.md`
- Issues: epic [#155](https://github.com/chatondearu/ai-dev-kit/issues/155),
  [#157](https://github.com/chatondearu/ai-dev-kit/issues/157),
  [#158](https://github.com/chatondearu/ai-dev-kit/issues/158),
  [#159](https://github.com/chatondearu/ai-dev-kit/issues/159),
  [#160](https://github.com/chatondearu/ai-dev-kit/issues/160),
  [#161](https://github.com/chatondearu/ai-dev-kit/issues/161),
  [#162](https://github.com/chatondearu/ai-dev-kit/issues/162),
  [#163](https://github.com/chatondearu/ai-dev-kit/issues/163)
- [[ADR-004-cdamux|ADR-004]], [[ADR-005-cda-graph-orchestration|ADR-005]],
  [[ADR-002-orch-auto-and-fast-lane|ADR-002]]
