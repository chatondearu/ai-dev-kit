---
name: cda-graph
description: >-
  Graph-first orchestration for CDA: parallel workers with structured outputs,
  integrator synthesize, foam-grounded verifier, live .orch/graph/live/<run_id>.json
  for cdamux. Use instead of brute-force single-context loops for multi-file or
  multi-issue work. Complements cda-dev (orch.sh) and OpenClaw Swarm patterns.
---

# CDA graph orchestration

Operator reference: [doc/graph-orchestration.md](../../../doc/graph-orchestration.md).

## Mental model (OpenClaw-aligned)

| Layer | Role |
| ----- | ---- |
| **Motion** | Short bounded nodes: implement → verify → (optional) fix |
| **Structure** | DAG: issues `Blocked by:`, plan tasks, foam ADRs |
| **Fan-out** | One worker per disjoint scope (issue or `--graph-plan` task) |
| **Fan-in** | Integrator reads structured JSON, writes `integrator-report.md` |
| **Grounding** | Verifier node checks plan/ADR (`ORCH_GRAPH_VERIFY=1`) |

Loops are for **convergence** (one PR, CI, TDD) — not for carrying a whole epic in one chat.

## Cookbook

### 1 — Multi-issue ready pickup

```bash
SKILL=~/.cursor/skills/cda-dev
export ORCH_REPO=/path/to/project
cd "$ORCH_REPO"

bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
# → .orch/graph/live/<run_id>.json (current.json → latest run), integrator when ≥2 issues
```

### 2 — Grounding verifier (foam / plan)

```bash
ORCH_GRAPH_VERIFY=1 bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12
```

Worker must create `.orch/verifier-ok` or `.orch/verifier-fail` in the worktree.

### 3 — Intra-issue graph plan

```bash
# Copy and edit:
cp ~/.cursor/skills/cda-graph/templates/graph-plan.example.json plans/graph/PLAN-42-tasks.json

bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh plans/graph/PLAN-42-tasks.json

# Parallel when paths are disjoint (default ORCH_GRAPH_PLAN_PARALLEL=auto):
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 42 \
  --graph-plan plans/graph/PLAN-42-tasks.json

# Force sequential:
ORCH_GRAPH_PLAN_PARALLEL=sequential bash "$SKILL/scripts/orch.sh" ...
```

Each task writes `.orch/task-<id>-result.json` in the shared worktree.

`paths` is enforced: a file changed outside a task's `paths` marks that task `blocked` with reason `paths: <files>`.

### 4 — Watch the graph

- **cdamux** TUI: Fleet → **project** or session → sidecar **Orch graph**; expand project for `⚙` worker rows (`.orch/run/status.json`); `o` opens GitHub issue from `feat/<n>-*`; `g` = full-screen drawn DAG + steps bar of the selected worker (`G` cycles `group_by`)
- `ORCH_TMUX=1` (opt-in, ignored with `--sandbox`): workers run in tmux sessions `orch-<run>-<worker>` linked by `@cdamux_parent` / `@cdamux_run` / `@cdamux_role`, previewed live in Fleet; `ORCH_TMUX_KEEP=1` keeps them after exit. CLI sub-agents appear as child rows when `./install.sh --cdamux-hooks` is on ([ADR-007](../../../foam/decisions/ADR-007-cdamux-session-hierarchy.md))
- Hooks: `tmux set-option -t SESSION @cdamux_state running|waiting|done|error` (Fleet prefers over pane heuristics)
- One live file per run: `.orch/graph/live/<run_id>.json` (`session`, `pid`); `current.json` is a symlink to the latest run, for readers that only know it
- Archives: `orch_graph_finish` moves the run to `.orch/graph/runs/<run_id>.json` (shown as recent runs); stale live files (dead pid, 24 h) move there on the next init
- **API**: `GET /v1/orch-graph?repo=` · `GET /v1/orch-status?repo=` · `GET /v1/orch-runs?repo=`

### 5 — Disable graph file

```bash
bash "$SKILL/scripts/orch.sh" --no-graph --mode ready-pickup --issue 12
# or ORCH_GRAPH=0
```

## Harness flags (summary)

| Flag / env | Effect |
| ---------- | ------ |
| `--no-graph` / `ORCH_GRAPH=0` | No live graph updates (same worker loop, verifier still runs) |
| `ORCH_GRAPH_VERIFY=1` | Verifier agent after verify |
| `ORCH_GRAPH_INTEGRATOR=1` | Integrator even for one issue |
| `ORCH_GRAPH_PLAN_PARALLEL=auto` | Parallel graph-plan tasks if paths disjoint |
| `ORCH_GRAPH_PLAN_PARALLEL=parallel` | Parallel; fail if paths overlap |
| `ORCH_GRAPH_PLAN_MAX_PARALLEL` | Cap parallel plan tasks (default `ORCH_MAX_PARALLEL`) |
| `--graph-plan FILE` | Intra-issue task fan-out; a task may set `tier` (`small\|medium\|large`) as its start tier |
| `ORCH_TIER_MAX` | Escalation ceiling (default `large`); a failed iteration re-runs one tier up |

## JSON schemas

| File | Purpose |
| ---- | ------- |
| [templates/graph-plan.schema.json](templates/graph-plan.schema.json) | `--graph-plan` |
| [templates/orch-graph.schema.json](templates/orch-graph.schema.json) | `.orch/graph/live/<run_id>.json` (and `current.json`) |
| [../cda-dev/templates/worker-result.schema.json](../cda-dev/templates/worker-result.schema.json) | Worker / task output |

## Cursor session fallback

Without `orch.sh`, **do not hand-write** graph files. Use `graph-node.sh` (same
`init`/`node`/`edge`/`finish`; mirrors via `cdamux state` when available). `init` starts
a new run and prints its file; `node` / `edge` / `finish` target `--run-id`, else
`$ORCH_RUN_ID`, else the newest live run of the current tmux session (outside tmux,
pass `--run-id <basename of the init output>`):

```bash
G=~/.cursor/skills/cda-graph/scripts/graph-node.sh
bash "$G" init --repo .
bash "$G" node --id integrator --kind integrator --phase pending --status queued --message "awaiting tasks"
# one node per parallel Task (disjoint scopes):
bash "$G" node --id task-api --kind worker --phase implement --status running --message "API" --parent integrator
# … after each Task finishes:
bash "$G" node --id task-api --kind worker --phase done --status done --message "ok" --parent integrator
bash "$G" finish --outcome ok
```

Steps:

1. `graph-node.sh init` then dispatch parallel Task subagents (one disjoint scope each).
2. Each Task writes `.orch/worker-result.json` / task result JSON per [worker-result.schema.json](../cda-dev/templates/worker-result.schema.json) and updates its graph node via `graph-node.sh node`.
3. One integrator subagent + `cda-review` on the combined diff; then `graph-node.sh finish`.

## References

- Skill `cda-dev` — harness owner
- ADR [[ADR-005-cda-graph-orchestration|ADR-005]]
- Orca crosswalk: [doc/orca-mapping.md](../../../doc/orca-mapping.md)
- [foam/graph/README.md](../../../foam/graph/README.md)
- OpenClaw Swarm: program = orchestration (`Promise.all`, structured child results)
