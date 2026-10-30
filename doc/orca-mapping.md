# Orca ↔ CDA kit mapping

Cross-reference between [Orca](https://www.onorca.dev/) (Agent Development Environment,
[stablyai/orca](https://github.com/stablyai/orca)) and this repository: **kit**
(`cda-dev`, `cda-graph`, `cda-kanban`, foam) plus **cdamux** (tmux host tool, ADR-004).

Orca is **not** a dependency of the kit (ADR-001). This page is for operators comparing
tools and planning incremental parity where it pays off.

**Competitor matrix (cdamux UX):** [`cdamux/docs/research.md`](../cdamux/docs/research.md#orca-onorcadev).

## Product split (who owns what)

| Concern | Orca | CDA |
| ------- | ---- | --- |
| Desktop shell (editor, GPU terminal, browser) | Native app | tmux + cdamux TUI / tailnet web |
| Git worktrees per task | First-class UI + CLI | `orch.sh` worktrees; cdamux optional spawn |
| GitHub / Linear task UI | In-app | `gh` + Kanban skills; no in-app board |
| Multi-agent orchestration contract | Run / Task / Dispatch / inbox | `orch.sh` + `.orch/graph/current.json` |
| Project memory (ADRs, PRDs) | Notes / generic | foam + `cda-foam` |
| CI-green PR pipeline | Operator workflow | `cda-dev` verify loop, frontier, integrator |

## Orchestration model mapping

Orca’s supervised layer ([orchestration docs](https://www.onorca.dev/docs/cli/orchestration))
vs kit graph orchestration ([graph-orchestration.md](graph-orchestration.md), ADR-005).

| Orca concept | Role | CDA equivalent | Gap |
| ------------ | ---- | -------------- | --- |
| **Run** | Durable namespace + coordinator inbox | One `orch.sh` invocation; `run_id` in `current.json` | No persistent inbox across runs; no `run-create` CLI |
| **Task** | Spec, deps, status lifecycle | GitHub issue + optional `plans/graph/*.json` tasks | Status not stored as Orca enum; use issue state + `.orch/run/*.status` |
| **Dispatch** | One attempt; authority for completion | One worker worktree / one graph-plan task attempt | No `dispatchId`; retries are new agent iterations inside `orch_graph_worker` |
| **Message / Delivery** | FIFO inbox, `check --wait --ack` | None | No ack/replay protocol |
| **worker_done** | Required completion with `taskId` + `dispatchId` | `.orch/worker-result.json` (convention); `.orch/run/<id>.rc` | No stale-dispatch guard; schema not enforced in harness |
| **heartbeat** | Long-run liveness | None | Optional future: `.orch/run/<id>.heartbeat` |
| **ask / decision gate** | Block task until coordinator answers | `ORCH_GRAPH_VERIFY` verifier; human in chat | No `gate-create` / `gate-resolve` API |
| **worker-start** | Supervised agent on worktree | `orch_ready_pickup_one` + tmux agent (manual) or stub in tests | cdamux spawns sessions; orch does not call cdamux |
| **Integrator** | Coordinator synthesis | `orch_graph_integrator_run` (2+ issues or graph-plan) | Same idea; different trigger rules |
| **Frontier / deps** | Task `blocked` until deps done | Issue body `Blocked by: #n` + `orch_frontier_partition` | Same dependency idea, GitHub-native |

## Host / session mapping (Orca desktop ↔ cdamux)

| Orca capability | cdamux today | Kit touchpoint |
| --------------- | ------------ | -------------- |
| Worktree sidebar | Fleet tree + optional `cdamux worktree list` | `.orch/worktrees/` (orch only; not auto-linked — see `cdamux/docs/architecture.md`) |
| Terminal send | `cdamux send`, Fleet composer | — |
| Parallel agents | Multiple `cdamux spawn` sessions | `orch.sh --max-parallel` |
| Preview / status | Pane capture, idle / needs_input | — |
| Diff review → agent | `cdamux review` (hunks) | `cda-review` on PR |
| Inline diff line comments | Rich diff UI | Hunk-based review only |
| Orch graph view | Sidecar + `GET …/orch-graph` | Writes `.orch/graph/current.json` |
| SSH remote worktrees | Tailnet API (read/send); Mosh doc | No remote file/git IDE |
| Mobile app | Web UI on tailnet (partial) | — |
| GitHub / Linear in-app | — | `cda-kanban`, `gh-board.sh` |
| Usage / account tracking | — | BYO subscriptions; no metering |
| `orca status --json` for agents | `cdamuxd` health + sessions API | No single “drive cdamux” skill shipped |
| Multi-repo parent folder chat | — | Multi-root discover in cdamux config |

## Command / workflow cheatsheet

Rough “same intent” paths (Orca → CDA). Orca commands require the Orca app + experimental orchestration where noted.

| Intent | Orca (reference) | CDA |
| ------ | ---------------- | --- |
| One-shot prompt in existing pane | `orca terminal send` | `cdamux send <session> "…"` |
| New isolated branch + agent | `orca worktree create` / `worker-start --worktree new-child` | `orch.sh --mode ready-pickup --issue N` or spawn modal + git worktree |
| Supervised worker with completion | `orca orchestration worker-start` … `worker_done` | Worker prompt + `worker-result.json`; graph nodes in `current.json` |
| Wait for workers | `orca orchestration check --wait` | Poll `.orch/run/*.rc` or watch cdamux Orch graph |
| Multi-issue parallel + merge story | Run + tasks + workers | `orch.sh --issue A --issue B` → integrator |
| Intra-issue parallel (disjoint paths) | Multiple tasks / workers | `--graph-plan plans/graph/….json` |
| DAG blocked on dependency | Task deps / gates | `Blocked by:` on issue body |
| Grounding vs plan/ADR | Coordinator review | `ORCH_GRAPH_VERIFY=1` |
| Disable live graph file | — | `ORCH_GRAPH=0` / `--no-graph` |
| Cursor chat only, no harness | — | `graph-node.sh` + parallel Tasks + `worker-result.schema.json` + integrator ([graph-orchestration.md](graph-orchestration.md)) |

## What we deliberately do not mirror

- Full ADE (embedded browser, design mode, VS Code-class editor).
- Orca’s message inbox and dispatch IDs (high implementation cost; tmux-centric model).
- Native iOS/Android app (tailnet web is the lightweight remote path).
- Linear UI (GitHub-first kit).
- Hosted Orca cloud VMs (operator-owned machines only).

## Suggested follow-ups

Shipped / in progress (visibility slice):

1. ~~Warn on `worker-result.json` in `orch.sh`~~ — `orch_worker_result_warn`.
2. ~~Worker preamble~~ — `cda-dev/prompts/worker.md`.
3. ~~`graph-node.sh` + schema for Cursor fallback~~ — avoids hand-written invalid graphs.
4. ~~cdamux schema errors (422) + stale badge~~ — `OrchGraphError` / `format_orch_graph_panel_ex`.
5. ~~bash `export -f` for graph helpers~~ — workers actually write nodes under `bash -c`.

Still open (P1+):

1. cdamux: GitHub issue link from branch `feat/<n>-*` in Fleet chrome.
2. Fleet rows from `.orch/run/status.json` (workers without tmux).
3. Project-level orch graph (no session required).
4. Atomic graph writes + sandbox bind for `.orch/graph`.

## References

- [Orca — What is Orca?](https://www.onorca.dev/docs)
- [Orca — Orchestration CLI](https://www.onorca.dev/docs/cli/orchestration)
- [Orca — Supported agents](https://www.onorca.dev/docs/agents/supported)
- ADR-001 (host tools boundary), ADR-004 (cdamux), ADR-005 (graph orchestration)
- [`doc/graph-orchestration.md`](graph-orchestration.md), [`doc/orchestration.md`](orchestration.md)
