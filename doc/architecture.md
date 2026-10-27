# Architecture — agent orchestration

## Components

| Piece | Role |
| ----- | ---- |
| `cda-triage` | Turns inbound issues into Ready + durable **agent brief** (or needs-info / human) |
| `orch.sh` | Kanban modes: intake, **ready-pickup**, pr-fix — worktrees, graph motion nodes, PR, CI watch |
| `cda-graph` | Model + foam export; live `.orch/graph/current.json` (see [graph-orchestration.md](graph-orchestration.md)) |
| `orch-auto.sh` | Opt-in poller: discover Ready → call `ready-pickup` |
| `headless.sh` | One prompt + verify in the current tree — **no** issue / worktree / PR |
| Host agent CLI | `agent` / `cursor-agent` / `claude` / `opencode` (or `ORCH_AGENT`) |

Boundary (accepted ADRs):

- **ADR-001** — Host apps stay outside the kit; headless ≠ Kanban.
- **ADR-002** — Fast-lane + orch-auto; do not teach headless `--issue` / open PR.

## Ready → PR flow

```mermaid
flowchart TD
  issue[GitHub_issue] --> triage[cda_triage]
  triage -->|agent_brief| ready[Board_Ready]
  triage -->|fuzzy_feature| prd[PRD_plus_PLAN]
  prd --> ready
  ready --> auto[orch_auto_optional]
  ready --> orch[orch_ready_pickup]
  auto --> orch
  orch --> lane{PLAN_file?}
  lane -->|yes| full[LANE_full]
  lane -->|no| fast[LANE_fast_brief]
  full --> pr[PR_CI_green]
  fast --> pr
  pr --> human[Human_merge]
```

Autonomy stops at a **mergeable, CI-green PR**. No auto-merge.

## Lanes (`ready-pickup`)

| Condition | Lane | Contract |
| --------- | ---- | -------- |
| `plans/PLAN-<id>-*.md` exists | `full` | Plan (+ epic PRD when present) |
| No plan; triage comment contains `## Agent Brief` | `fast` | Agent brief only |
| No plan and no brief | — | Worker blocked |

Bugs / bounded scope may go Ready with brief alone. Fuzzy enhancements still
need PRD → plan before Ready.

## Graph motion (ready-pickup)

```mermaid
flowchart LR
  subgraph workers [Fan-out]
    W1[worker issue A]
    W2[worker issue B]
  end
  subgraph nodes [Per worker]
    I[implement]
    V[verify]
    F[fix optional]
  end
  W1 --> I --> V
  V -->|fail| F --> I
  W2 --> I
  W1 --> INT[integrator]
  W2 --> INT
  INT --> PR[PR plus CI]
```

Multi-issue runs write `.orch/graph/current.json`. Intra-issue fan-out uses
`--graph-plan` (parallel when task `paths` are disjoint). Details:
[graph-orchestration.md](graph-orchestration.md).

## What headless is for

Local one-shot tasks (“fix this test”) with graph motion nodes when `ORCH_GRAPH=1`.
It does **not** claim board cards or open PRs. Use orch / orch-auto for ticket-backed work.
