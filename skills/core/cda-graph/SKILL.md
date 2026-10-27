---
name: cda-graph
description: >-
  Graph-first orchestration for CDA: parallel workers with structured outputs,
  integrator synthesize, foam-grounded verifier, live .orch/graph/current.json
  for cdamux. Use instead of brute-force single-context loops for multi-file or
  multi-issue work. Complements cda-dev (orch.sh) and OpenClaw Swarm patterns.
---

# CDA graph orchestration

Announce: **"Using cda-graph skill."**

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
# → .orch/graph/current.json, integrator when ≥2 issues
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

### 4 — Foam machine graph (no VS Code)

```bash
bash ~/.cursor/skills/cda-graph/scripts/foam-graph-export.sh --repo .
bash ~/.cursor/skills/cda-graph/scripts/foam-graph-export.sh --repo . --merge-orch
# → foam/graph/export.json
```

### 5 — Watch the graph

- **cdamux** TUI: Fleet → session → sidecar **Orch graph**
- **cdamuxd** web: session → **Orch graph** tab (`GET /v1/sessions/{name}/orch-graph`)

### 6 — Disable graph file

```bash
bash "$SKILL/scripts/orch.sh" --no-graph --mode ready-pickup --issue 12
# or ORCH_GRAPH=0
```

## Harness flags (summary)

| Flag / env | Effect |
| ---------- | ------ |
| `--no-graph` / `ORCH_GRAPH=0` | No `current.json` updates |
| `ORCH_GRAPH_VERIFY=1` | Verifier agent after verify |
| `ORCH_GRAPH_INTEGRATOR=1` | Integrator even for one issue |
| `ORCH_GRAPH_PLAN_PARALLEL=auto` | Parallel graph-plan tasks if paths disjoint |
| `ORCH_GRAPH_PLAN_PARALLEL=parallel` | Parallel; fail if paths overlap |
| `ORCH_GRAPH_PLAN_MAX_PARALLEL` | Cap parallel plan tasks (default `ORCH_MAX_PARALLEL`) |
| `--graph-plan FILE` | Intra-issue task fan-out |

## JSON schemas

| File | Purpose |
| ---- | ------- |
| [templates/graph-plan.schema.json](templates/graph-plan.schema.json) | `--graph-plan` |
| [../cda-dev/templates/worker-result.schema.json](../cda-dev/templates/worker-result.schema.json) | Worker / task output |

## Cursor session fallback

Without `orch.sh`:

1. Dispatch parallel Task subagents (one disjoint scope each).
2. Collect JSON summaries per [worker-result.schema.json](../cda-dev/templates/worker-result.schema.json).
3. One integrator subagent + `cda-review` on the combined diff.

## References

- Skill `cda-dev` — harness owner
- ADR [[ADR-005-cda-graph-orchestration|ADR-005]]
- [foam/graph/README.md](../../../foam/graph/README.md)
- OpenClaw Swarm: program = orchestration (`Promise.all`, structured child results)
