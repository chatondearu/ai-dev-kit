# Configuration

## Host CLIs

| Tool | Used by |
| ---- | ------- |
| Agent CLI (`agent`, `cursor-agent`, `claude`, `opencode`, `codex`) | `orch.sh`, `headless.sh` |
| `gh`, `git` | Kanban / CI modes of orch |
| `jq`, `flock` | `orch-auto.sh`, `orch.sh` (ready-pickup / pr-fix) |
| `bwrap` (required for `--sandbox`) | `--sandbox` exits if missing (no unsandboxed fallback) |

See [`docs/recommended.md`](../docs/recommended.md). The kit never installs
these (`./scripts/doctor.sh` only checks `PATH`). With a Cursor CLI present it also runs
`--list-models` once and warns for each `cursor` model id of the active tier table
(`ORCH_TIERS_FILE` or the kit default) that is not listed; warning only, exit status unchanged.

## Environment variables

| Variable | Default | Meaning |
| -------- | ------- | ------- |
| `ORCH_AGENT` | inherited | Agent CLI for every role. Unset → CLI of the session that launched `orch.sh` (`CLAUDECODE`, `CODEX_THREAD_ID`/`CODEX_SESSION_ID`, `CURSOR_INVOKED_AS`, else `ps` ancestry), then first of `agent`, `cursor-agent`, `claude`, `opencode` on `PATH` ([ADR-010](../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers.md)). **Required** by `orch-auto.sh` (exit 2 when empty) |
| `ORCH_AGENT_<ROLE>` | unset | Per-role CLI, beats `ORCH_AGENT`: `ORCH_AGENT_WORKER`, `ORCH_AGENT_REVIEWER`, `ORCH_AGENT_VERIFIER`, `ORCH_AGENT_INTEGRATOR`, `ORCH_AGENT_PR_FIX` |
| `ORCH_TIER_<ROLE>` | `medium` (`small` for `integrator`) | Per-role tier (`small`, `medium`, `large`): `ORCH_TIER_WORKER`, `ORCH_TIER_REVIEWER`, `ORCH_TIER_VERIFIER`, `ORCH_TIER_INTEGRATOR`, `ORCH_TIER_PR_FIX`. A tier maps to model/effort flags per CLI in [`config/tiers.tsv`](../skills/core/cda-dev/config/tiers.tsv). An agent command that already passes `--model` / `-m` is `pinned` and gets no tier flags |
| `ORCH_TIER` | unset | Current tier for every role, beats `ORCH_TIER_<ROLE>`. The worker / pr-fix loop sets it on escalation; a graph-plan task `tier` sets it as the start tier |
| `ORCH_TIER_MAX` | `large` | Escalation ceiling (`small`, `medium`, `large`; anything else → `orch.sh` exits 2). A red verify or failed grounding verifier re-runs the next iteration one tier up; a failure at the ceiling ends the worker `blocked` (`tier ceiling <tier> reached: <failure>`) even with iterations left. Pinned commands and CLIs with no table row never escalate. The review gate keeps its single fix iteration |
| `ORCH_TIERS_FILE` | kit `config/tiers.tsv` | Replaces the whole tier table (`cli<TAB>tier<TAB>flags`, `#` comments; `cursor` rows serve `agent` and `cursor-agent`). Missing / unreadable file → `orch.sh` exits 2. A CLI with no row runs without flags and warns once per run |
| `ORCH_ROLE` | `worker` | Internal: role of the current agent call, set by the harness at each call site (`pr-fix` loop runs with `ORCH_ROLE=pr-fix`) |
| `ORCH_MAX_PARALLEL` | `3` (`1` for orch-auto default) | Parallel ready-pickup workers |
| `ORCH_MAX_ITER` | `10` | Verify-loop retries per worker |
| `ORCH_PROGRESS` | `auto` | Set `0` to disable TTY dashboard |
| `ORCH_SANDBOX` | off | `1` / `--sandbox` → bubblewrap required |
| `ORCH_TMUX` | `0` | `1` → each worker's agent runs in a detached tmux session `orch-<run>-<worker>` (`@cdamux_*` options, log `.orch/run/<worker>.log`); ignored with `--sandbox` (the tmux server runs outside bwrap); the agent env reaches it through a self-deleting 0600 runner file, see [ADR-007](../foam/decisions/ADR-007-cdamux-session-hierarchy.md) |
| `ORCH_TMUX_KEEP` | `0` | `1` → keep the worker session after the agent exits (state `done` / `error`) |
| `ORCH_ALLOW_NO_CHECKS` | `0` | `1` → empty PR checks count as green. Without it, empty checks fail-fast as `none` (no 600s wait). Also enabled by `orch: allow-no-checks` in `AGENTS.md`. |
| `ORCH_COPY_ENV` | `0` | `1` → copy `.env` into worktrees |
| `ORCH_VERIFY` | unset | Override verify command (allowlisted under pr-fix) |
| `ORCH_FRONTIER` | `auto` | `0` disables Blocked-by gating |
| `ORCH_FORCE_CLEAN` | off | Remove worktrees after success |
| `ORCH_REPO` | cwd / `--repo` | Target git tree (required in systemd env) |
| `ORCH_GH_BOARD` | discovered | Path to `gh-board.sh` for orch-auto |
| `CDA_DESIGN_LOOP` | off | Opt-in design loop for UI diffs |
| `ORCH_GRAPH` | `1` | Live `.orch/graph/live/<run_id>.json` (`0` / `--no-graph` off; the worker loop and verifier still run) |
| `ORCH_GRAPH_VERIFY` | `1` | foam/plan verifier agent after local verify; `0` disables |
| `ORCH_GRAPH_INTEGRATOR` | `auto` | Integrator after workers (`1` always, `0` never) |
| `ORCH_GRAPH_PLAN_PARALLEL` | `auto` | Graph-plan tasks: `auto` / `parallel` / `sequential` |
| `ORCH_GRAPH_PLAN_MAX_PARALLEL` | `$ORCH_MAX_PARALLEL` | Parallel cap for `--graph-plan` tasks |
| `CDA_GRAPHIFY` | unset | `1` when `cda.aitools.graphify.enable` → kit skills and the kiss SessionStart hook prefer `graphify-out/` before grep/file reads ([ADR-008](../foam/decisions/ADR-008-graphify-code-graph.md); never set by `install.sh`) |
| `KISS_MAP` | on | `0` → skip SessionStart orientation (repo map / graphify pointer) |
| `CDA_FOAM_HOME` | unset | Shared foam home (`cda.aitools.foamHome`); `cda-floorplan` writes `$CDA_FOAM_HOME/projects/<repo>/floorplan/` when the project has no `foam/` |
| `FLOORPLAN_ENGINE` / `FLOORPLAN_AGENT` | auto | Override Ask / build agent for `cda-floorplan` (`agent`, `cursor-agent`, `claude`, `opencode`, `codex`) |
| `CDA_AITOOLS_HOME` | `~/.config/cda-aitools` | Companion config root (`cda.aitools.configRoot` or env) |
| `CDAMUX_HOME` | `${XDG_STATE_HOME:-~/.local/state}/cdamux` | cdamux state root (`agents/`, `runs/`, restore registry, …). [ADR-011](../foam/decisions/ADR-011-cdamux-unified-state-model.md) |
| `CDAMUX_CONFIG` | `${XDG_CONFIG_HOME:-~/.config}/cdamux/config.toml` | Override path to the cdamux TOML config file |
| `CDAMUX_AGENT_ID` | unset (set by cdamux at spawn) | Stable agent id (`a-xxxxx`) exported into the agent process / pane env |

### cdamux state and inbox

Hooks, orch and workers write agent/run JSON only through `cdamux state set|get|append`
under `$CDAMUX_HOME` (format private). `orch.sh` / `graph-node.sh` dual-write the run
(+ nodes) and one Agent per worker there when `cdamux` is on `PATH`; missing binary →
silent no-op (in-repo `.orch/graph` and `.orch/run/status.json` remain until #252).
With `$CDAMUX_AGENT_ID` set, the orch run records that agent as `parent` (sub-run).
Orchestrator ↔ worker messages use `cdamux msg send|read` (`inbox/<agent-id>.jsonl` +
`.cursor`, ≤ 2 KB/line). Start an orchestrator with `cdamux orch new` (or Fleet spawn
role *orchestrator*) — skill `cda-orchestrator`. Interactive workers:
`cdamux worker spawn|ls|peek` (needs `$CDAMUX_AGENT_ID`; worktrees under
`[worktrees].root` or `$CDAMUX_HOME/worktrees`; default first message includes the
worker preamble for `cdamux msg`). Layout, ids, delivery and restore:
[cdamux/docs/architecture.md](../cdamux/docs/architecture.md) § State root.

See [graph-orchestration.md](graph-orchestration.md).
Graphify install stays in **aitools** only — never `install.sh`
([ADR-001](../foam/decisions/ADR-001-host-tools-vs-kit.md)).

## `cda-floorplan` map location

Skill: `skills/tools/cda-floorplan/` (vendored Floorplan v0.3.0). Output, first match:

1. `<project>/foam/floorplan/` + note `foam/floorplan.md` when `foam/` exists
2. `$CDA_FOAM_HOME/projects/<repo>/floorplan/` (+ sibling `floorplan.md`) when `CDA_FOAM_HOME` is set
3. else `<project>/floorplan/` (kept fully out of git)

Under foam / `CDA_FOAM_HOME`, commit `architecture.json`, `descriptions/`, and the note; generated HTML/graph helpers are gitignored. No `CLAUDE.local.md` block.

## On-disk state (under the target repo)

| Path | Purpose |
| ---- | ------- |
| `.orch/worktrees/` | Per-issue worktrees |
| `.orch/run/` | Worker exit codes / progress |
| `.orch/graph/live/<run_id>.json` | Live orchestration graph, one per run; `current.json` symlinks the latest (cdamux / foam export) |
| `.orch/integrator-report.md` | Post-run synthesis (multi-worker) |
| `.orch/headless/` | Headless prompt copy |
| `.orch/auto.lock` | orch-auto mutex |
| `.orch/orch.lock` | orch ready-pickup / pr-fix mutex. Without `flock` it is the dir `.orch/orch.lock.d/` holding the owner `pid`; a dead pid is taken over |
| `.orch/worktrees/` | Per-issue worktrees (prune with `orch-cleanup.sh`) |
| `.orch/auto/*.log` | orch-auto run logs (newest 50 kept) |

## Systemd env file

Example: `skills/core/cda-dev/templates/systemd/cda-orch-auto.env.example` →
`~/.config/cda-orch-auto.env`. At minimum set `ORCH_REPO` to an absolute path.
Ensure agent CLIs and `gh` are on `PATH` for the user systemd session.

## `install.sh` and agent homes

`./install.sh` symlinks skills (and Cursor agents/plugins) into each enabled
tool's config dir. Override roots with `CURSOR_HOME`, `CLAUDE_CONFIG_DIR`, and
`XDG_CONFIG_HOME` (opencode).

### Cursor rules (manual)

Cursor does **not** auto-load kit rules from disk. Copy or paste the contents
of `rules/*.md` into **Cursor Settings → Rules for AI** yourself (see also
`rules/nix-develop-shell.md`). Per-repo workflow lives in the project's
`AGENTS.md`; `install.sh` does not copy `rules/` into `~/.cursor`.

### KISS hooks (`--hooks`, default on)

When hooks are enabled, `skills/quality/cda-kiss/hooks/` is merged into each
tool's hook config (needs `jq`; idempotent):

| Tool | Hook file | Events |
| ---- | --------- | ------ |
| Cursor | `$CURSOR_HOME/hooks.json` | `sessionStart`, `beforeSubmitPrompt` |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` | `SessionStart`, `SubagentStart`, `UserPromptSubmit` |
| opencode | `$XDG_CONFIG_HOME/opencode/plugin/kiss.mjs` | plugin (not JSON hooks) |

Claude assembles global instructions from `rules/` into `CLAUDE.md` when Claude
is enabled; that path is separate from KISS hooks.

When `CDA_GRAPHIFY=1` and `<repo>/graphify-out/graph.json` exists, the Claude /
Cursor SessionStart hook injects a pointer to `graphify-out/GRAPH_REPORT.md`
instead of the cheap repo map. Off or missing graph → repo map unchanged.
`rules/03-graphify.md` and code-reading skills carry the same rule.
