# Graph orchestration (`cda-graph`)

Graph-first runs replace **one fat agent loop** with explicit nodes: parallel
workers, short verify/fix motion, integrator fan-in, optional foam grounding.
OpenClaw Swarm uses the same idea (program orchestration, structured child output).

For comparison with **Orca** orchestration (Run / Task / Dispatch, `worker_done`), see
[Orca ↔ kit mapping](orca-mapping.md) — we map concepts; we do not implement Orca’s inbox.

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

# Foam export for agents without VS Code Foam
bash ~/.cursor/skills/cda-graph/scripts/foam-graph-export.sh --repo "$REPO" --merge-orch
```

## Runtime artifact

| Path | Content |
| ---- | ------- |
| `.orch/graph/current.json` | Live DAG: workers, implement/verify nodes, integrator |
| `foam/graph/export.json` | Wikilink graph + optional orch overlay (generated) |
| `.orch/integrator-report.md` | Human-readable synthesis after multi-worker runs |

## Environment variables

| Variable | Default | Meaning |
| -------- | ------- | ------- |
| `ORCH_GRAPH` | `1` | Write `current.json`; `0` / `--no-graph` disables |
| `ORCH_GRAPH_VERIFY` | `0` | `1` → verifier agent after local verify (plan/ADR grounding) |
| `ORCH_GRAPH_INTEGRATOR` | `auto` | `auto` = integrator when 2+ issues; `1` = always; `0` = never |
| `ORCH_GRAPH_PLAN_PARALLEL` | `auto` | `auto` = parallel tasks when paths disjoint; `parallel` = require disjoint; `sequential` = always serial |
| `ORCH_GRAPH_PLAN_MAX_PARALLEL` | `$ORCH_MAX_PARALLEL` | Cap parallel graph-plan tasks (default 3) |

## Graph plan JSON

Store plans under `plans/graph/` or `.orch/graph/` (gitignored). Example:
[`skills/core/cda-graph/templates/graph-plan.example.json`](../skills/core/cda-graph/templates/graph-plan.example.json).

**Parallel safety:** tasks must not share overlapping `paths` prefixes. Validate:

```bash
bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh my-plan.json
```

## Observability

| Surface | How |
| ------- | --- |
| **cdamux TUI** | Fleet → select **project** or session → sidecar **Orch graph**; expand project for `⚙ #id` workers from `.orch/run/status.json` |
| **cdamuxd web** | Tailnet dashboard → session → **Orch graph** (canvas, polls API) |
| **API** | `GET /v1/sessions/{tmux-name}/orch-graph` |
| **API (project)** | `GET /v1/orch-graph?repo=` · `GET /v1/orch-status?repo=` · `GET /v1/orch-runs?repo=` (archived finishes under `.orch/graph/runs/`) |
| **Session hooks** | `tmux set-option -t SESSION @cdamux_state running\|waiting\|done\|error` — cdamux Fleet prefers this over pane heuristics |
| **GitHub issue** | Fleet `o` opens `gh issue view N --web` when branch is `feat/<n>-*` or worker id is numeric |

## Cursor-only (no harness)

**Do not invent** `.orch/graph/current.json` by hand — cdamux requires `run_id` +
`nodes`/`edges` ([orch-graph.schema.json](../skills/core/cda-graph/templates/orch-graph.schema.json)).

```bash
G=~/.cursor/skills/cda-graph/scripts/graph-node.sh
bash "$G" init --repo "$REPO"
# update nodes as Tasks run; then:
bash "$G" finish --outcome ok
```

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
