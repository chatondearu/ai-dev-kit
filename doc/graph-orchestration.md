# Graph orchestration (`cda-graph`)

Graph-first runs replace **one fat agent loop** with explicit nodes: parallel
workers, short verify/fix motion, integrator fan-in, optional foam grounding.
OpenClaw Swarm uses the same idea (program orchestration, structured child output).

For comparison with **Orca** orchestration (Run / Task / Dispatch, `worker_done`), see
[Orca ↔ kit mapping](orca-mapping.md) — kit delivery is `cdamux msg` ([ADR-011](../foam/decisions/ADR-011-cdamux-unified-state-model.md) §4), not Orca’s inbox API.

## Quick start

```bash
SKILL=~/.cursor/skills/cda-dev
REPO=/path/to/your/project

# Multi-issue pickup (integrator auto when 2+ issues)
bash "$SKILL/scripts/orch.sh" --repo "$REPO" --mode ready-pickup --issue 12 --issue 15

# Intra-issue tasks (validate plan first)
bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh ./plans/graph/issue-42.json
bash "$SKILL/scripts/orch.sh" --repo "$REPO" --mode ready-pickup --issue 42 \
  --graph-plan ./plans/graph/issue-42.json
```

## Runtime artifact

| Path | Content |
| ---- | ------- |
| `.orch/graph/live/<run_id>.json` | Live DAG of one run: workers, implement/verify nodes, integrator; `session` (creator's tmux `#S`) and `pid` |
| `.orch/graph/current.json` | Compatibility symlink to the latest run (`live/` while running, `runs/` once finished) |
| `.orch/graph/runs/<run_id>.json` | Finished runs (last 20); `live/` files with a dead pid and no write for 24 h move here on the next init |
| `.orch/integrator-report.md` | Human-readable synthesis after multi-worker runs |

Nodes of an agent run (implement / fix, verifier, integrator) carry `cli`, `model`,
`effort` and `tier` ([ADR-010](../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers.md)).
The same four optional fields sit on each `.orch/run/status.json` worker entry; cdamux shows
`[cli·tier]` on the Fleet worker row and `model` / `effort` in the worker sidecar.

## Environment variables

| Variable | Default | Meaning |
| -------- | ------- | ------- |
| `ORCH_GRAPH` | `1` | Write the live graph; `0` / `--no-graph` disables |
| `ORCH_GRAPH_VERIFY` | `1` | Verifier agent after local verify (plan/ADR grounding); `0` disables (ADR-006) |
| `ORCH_GRAPH_INTEGRATOR` | `auto` | `auto` = integrator when 2+ issues; `1` = always; `0` = never |
| `ORCH_GRAPH_PLAN_PARALLEL` | `auto` | `auto` = parallel tasks when paths disjoint; `parallel` = require disjoint; `sequential` = always serial |
| `ORCH_GRAPH_PLAN_MAX_PARALLEL` | `$ORCH_MAX_PARALLEL` | Cap parallel graph-plan tasks (default 3) |
| `ORCH_TMUX` | `0` | `1` = each worker's agent runs in a tmux session `orch-<run>-<worker>` (see below); ignored with `--sandbox` |
| `ORCH_TMUX_KEEP` | `0` | `1` = keep the worker session after the agent exits (`@cdamux_state` `done` / `error`) |

### Workers in tmux (`ORCH_TMUX=1`)

Opt-in. Each worker's agent runs in a detached tmux session so cdamux can preview
its pane and you can attach with `tmux attach -t orch-<run>-<worker>`. Options set
on the session: `@cdamux_run` (`ORCH_RUN_ID`), `@cdamux_role=worker`,
`@cdamux_agent`, `@cdamux_state` (mirrors the phase), and `@cdamux_parent` when orch
itself runs inside tmux (the worker then nests under that session in Fleet).
`.orch/run/status.json` entries gain `tmux_session` and `log`
(`.orch/run/<worker>.log`, the agent output, used as preview once the session is
gone). The session is killed when the agent exits unless `ORCH_TMUX_KEEP=1`; Ctrl-C
kills the run's sessions. `--sandbox` ignores `ORCH_TMUX` because the tmux server
runs outside bubblewrap. Decision: [ADR-007](../foam/decisions/ADR-007-cdamux-session-hierarchy.md).

Interactive `cdamux worker spawn|ls|peek` (needs `$CDAMUX_AGENT_ID`) is separate from
harness `ORCH_TMUX=1`: the CLI spawns human-driven panes under the caller's
orchestrator; orch.sh still owns Ready-pickup workers until slice 3 publishes them.

## Graph plan JSON

Store plans under `plans/graph/` or `.orch/graph/` (gitignored). Example:
[`skills/core/cda-graph/templates/graph-plan.example.json`](../skills/core/cda-graph/templates/graph-plan.example.json).

Each task may set `tier` (`small`, `medium`, `large`): the tier its worker starts at
([ADR-010](../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers.md)); it still
escalates one tier per failure up to `ORCH_TIER_MAX`. Unset → the role default.

**Parallel safety:** tasks must not share overlapping `paths` prefixes. Validate:

```bash
bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh my-plan.json
```

**Paths are enforced:** after a task's worker returns, any changed file outside its `paths` (and outside `.orch/`) sets the task to `blocked` with reason `paths: <files>`; tasks without `paths` are unrestricted.

## Observability

| Surface | How |
| ------- | --- |
| **cdamux TUI** | Fleet → select **project** or session → sidecar **Orch graph**; each live run is a `◈ run <id>` row under the session that started it (headless runs under the project), with its `⚙ #id` workers (`.orch/run/status.json` with the same `run_id`) and worker sessions; a session shows its own newest run (else its `@cdamux_run`), a project the latest; `g` opens the full-screen drawn DAG with a steps bar (implement / verify / review / CI / PR) for the selected worker, `Tab` there cycles a session's runs |
| **cdamux sessions** | `ORCH_TMUX=1` harness workers and interactive `cdamux worker spawn` panes are tmux sessions linked by `@cdamux_parent` / `@cdamux_run` / `@cdamux_role`; CLI sub-agents show as child rows (`cdamux/docs/subagents.md`, hooks via `./install.sh --cdamux-hooks`) |
| **cdamuxd web** | Tailnet dashboard → session → **Orch graph** (canvas, polls API) |
| **API** | `GET /v1/sessions/{tmux-name}/orch-graph` |
| **API (project)** | `GET /v1/orch-graph?repo=[&run=]` (default: latest run) · `GET /v1/orch-status?repo=` · `GET /v1/orch-runs?repo=` (live runs with `session`, then archived finishes under `.orch/graph/runs/`; `live` flag) |
| **Session hooks** | Opt-in `./install.sh --cdamux-hooks` writes agent status to `$CDAMUX_HOME/agents/<id>.json` via `cdamux state append` (Fleet prefers that) and injects an unread `cdamux msg` digest on turn-end (Claude `Stop` / Cursor `stop.followup_message`). Cursor `beforeSubmitPrompt` cannot inject context. Transitional `@cdamux_state` (set by `ORCH_TMUX=1`) remains a fallback before pane heuristics — see `cdamux/docs/subagents.md` |
| **GitHub issue** | Fleet `o` opens `gh issue view N --web` when branch is `feat/<n>-*` or worker id is numeric |

## Cursor-only (no harness)

**Do not invent** graph files by hand — cdamux requires `run_id` +
`nodes`/`edges` ([orch-graph.schema.json](../skills/core/cda-graph/templates/orch-graph.schema.json)).

```bash
G=~/.cursor/skills/cda-graph/scripts/graph-node.sh
bash "$G" init --repo "$REPO"          # prints .orch/graph/live/<run_id>.json
# update nodes as Tasks run; then:
bash "$G" finish --outcome ok
```

`init` always starts a new run. `node` / `edge` / `finish` write to `--run-id ID`,
else `$ORCH_RUN_ID` (set inside orch workers), else the newest live run created from
the current tmux session; otherwise they fail. Two sessions in the same repo, or a
session and an orch run, each keep their own file.

1. Split plan into disjoint file scopes; `graph-node.sh init`.
2. Dispatch parallel subagents (one scope each); each returns JSON like `worker-result.schema.json` and updates a graph node.
3. Integrator subagent reads summaries → one review (`cda-review`) on combined diff; `graph-node.sh finish`.

Skill: `cda-graph`. Harness owner: `cda-dev`. ADR: [ADR-005](../foam/decisions/ADR-005-cda-graph-orchestration.md).

## Headless single task

`headless.sh` uses the same graph motion nodes (`implement` / `verify` / `fix`) when
`ORCH_GRAPH=1` (default):

```bash
bash "$SKILL/scripts/headless.sh" --prompt "fix flaky test in pkg/foo" --max-iter 5
```
