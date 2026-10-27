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

## Mental model (OpenClaw-aligned)

| Layer | Role |
| ----- | ---- |
| **Motion** | Short bounded nodes: implement → verify → (optional) fix |
| **Structure** | DAG: issues `Blocked by:`, plan tasks, foam ADRs |
| **Fan-out** | One worker per disjoint scope (issue or `--graph-plan` task) |
| **Fan-in** | Integrator reads `worker-result.json`, writes `integrator-report.md` |
| **Grounding** | Verifier node checks plan/ADR (`ORCH_GRAPH_VERIFY=1`) |

Loops are for **convergence** (one PR, CI, TDD) — not for carrying a whole epic in one chat.

## Live runtime graph

`orch.sh` (default `ORCH_GRAPH=1`) writes:

```text
<repo>/.orch/graph/current.json
```

cdamux Fleet tab polls this when the selected project matches `repo` in the file.

## Machine-readable foam graph (outside VS Code)

Versioned knowledge stays in `foam/` wikilinks. For CLI/agents without Foam:

```bash
bash ~/.cursor/skills/cda-graph/scripts/foam-graph-export.sh --repo .
bash ~/.cursor/skills/cda-graph/scripts/foam-graph-export.sh --repo . --merge-orch
```

Outputs `foam/graph/export.json` (nodes/edges from wikilinks + optional live orch overlay).

See [foam/graph/README.md](../../../foam/graph/README.md).

## Harness flags

```bash
bash "$SKILL/cda-dev/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
# Integrator auto when 2+ issues

ORCH_GRAPH_VERIFY=1 bash ...   # grounding verifier after local verify
ORCH_GRAPH_INTEGRATOR=1 bash ...  # force integrator even for one issue
bash ... --no-graph            # disable current.json updates
bash ... --graph-plan .orch/graph/plan.json  # intra-issue fan-out (opt-in)
```

## Intra-issue plan (`--graph-plan`)

JSON array of tasks with non-overlapping `paths`:

```json
{
  "issue": 42,
  "tasks": [
    { "id": "api", "paths": ["src/api/"], "prompt": "..." },
    { "id": "ui", "paths": ["src/components/"], "prompt": "..." }
  ]
}
```

Workers share one branch; integrator merges narrative (human resolves git conflicts if any).

## Cursor session fallback

Without `orch.sh`, mirror the graph manually:

1. Dispatch parallel Task subagents (one scope each) with [dispatching-parallel-agents](https://github.com/obra/superpowers).
2. Collect structured JSON summaries.
3. One integrator subagent + `cda-review` on the combined diff.

## References

- Skill `cda-dev` — harness owner
- ADR [[ADR-005-cda-graph-orchestration|ADR-005]]
- OpenClaw Swarm: program = orchestration (`Promise.all`, structured child results)
