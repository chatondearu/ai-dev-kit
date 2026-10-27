# ADR-005 — CDA graph orchestration

- **Status**: accepted
- **Date**: 2026-10-27
- **Tags**: #architecture #tooling #autonomy

## Context

Single-context agent loops are costly and error-prone for multi-file / multi-issue work.
OpenClaw documents **Swarm** orchestration (parallel children, structured results,
program-as-graph) and separates **loop motion** from **durable graph state**.
The kit already had parallel `orch.sh` worktrees and foam wikilinks but no shared
runtime graph or integrator/verifier nodes.

## Decision

1. **`cda-graph` skill** documents the canonical model: fan-out workers, structured
   `worker-result.json`, integrator synthesize, optional foam-grounded verifier.
2. **`orch.sh`** writes live state to `<repo>/.orch/graph/current.json` by default
   (`ORCH_GRAPH=1`). Integrator runs automatically when dispatching **2+** issues.
3. **Foam machine export** via `foam-graph-export.sh` → `foam/graph/export.json`
   (wikilinks + optional orch overlay). No new graph database in the kit.
4. **cdamux** displays the live orch graph in Fleet (read-only); execution stays in bash harness.
5. **OpenClaw** remains external (ADR-001); document pattern mapping only.
6. **Loops** remain for CI, TDD, headless one-shot — implemented as **short graph nodes**
   (`implement` / `verify` / `fix`) inside `orch_graph_worker`, not one opaque while-loop.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Vendor LangGraph / OpenClaw in-tree | ADR-001 boundary; portability |
| Replace all loops including `loop-on-ci` | Different problem domain |
| Neo4j / custom graph DB | Operational weight; foam + JSON files suffice |

## Consequences

- **Positive**: Inspectable runs; cdamux visibility; aligns with Swarm mental model.
- **Negative**: More agent calls when `ORCH_GRAPH_VERIFY=1`; jq required for graph JSON updates.
- **Shipped follow-up**: `--graph-plan` intra-issue fan-out; cdamuxd `/v1/sessions/{id}/orch-graph` + web canvas.

## References

- Skill `skills/core/cda-graph/SKILL.md`
- `skills/core/cda-dev/scripts/lib/graph.sh`
- OpenClaw Swarm docs (program orchestration, structured child results)
