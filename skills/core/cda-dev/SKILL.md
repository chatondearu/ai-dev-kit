---
name: cda-dev
description: >-
  Unified meta-orchestrator: intake, Ready pickup, or PR-fix through foam/kanban,
  git worktrees, verify loops, and CI-green PRs (no auto-merge). Portable across
  Cursor, Claude Code, and opencode via orch.sh harness or skill-only fallback.
  Use for multi-subject batches, parallel features, or unblocking red CI.
---

# Dev orchestrator

Announce: **"Using cda-dev skill."**

## Role

**Glue master** — orchestrates foam, Kanban, worktrees, workers, verify, and CI.
Never edits product application source; workers do. Holds ledger + ids + paths,
not full worker diffs.

| Phase | Owns | Delegates to |
| ----- | ---- | ------------ |
| Resolve / artifacts | Mode-dependent bootstrap | `cda-foam`, `cda-prd`, `cda-plan`, `cda-kanban` |
| Dispatch | Parallel workers in worktrees | [prompts/worker.md](prompts/worker.md), [prompts/reviewer.md](prompts/reviewer.md) |
| Verify loop | Local verify + review + CI watch | `scripts/orch.sh`, `cda-review`, project `AGENTS.md` |
| Aggregate | Ledger + human report | [templates/run-ledger.md](templates/run-ledger.md) |

Details: [conventions.md](conventions.md). Design:
[foam/archive/superpowers/2026-10-02-dev-orchestrator-design.md](../../../foam/archive/superpowers/2026-10-02-dev-orchestrator-design.md)
(historical; operator truth in `doc/orchestration.md`).

## Modes

| Mode | Trigger | Human GO | Bootstrap | Exit |
| ---- | ------- | -------- | --------- | ---- |
| `intake` | Multi-subject chat list | Yes (epic breakdown) | Epic issue + approved PRD + N Ready children + N plans | PRs CI-green |
| `ready-pickup` | Ready Kanban cards | No | Plan **or** triage agent brief (fast-lane) | PRs CI-green |
| `pr-fix` | PR URL/number | No | None | Same PR CI-green |

**Lanes (ready-pickup):** if `plans/PLAN-<id>-*` exists → `full` (plan/PRD).
Else → `fast` (triage agent brief required). See foam ADR-002.

Intake Q&A: [prompts/intake.md](prompts/intake.md). PR repair: [prompts/pr-fix.md](prompts/pr-fix.md).
Epic table: [templates/epic-breakdown.md](templates/epic-breakdown.md).

## Hard rules

1. Master **never** edits product application source; workers do.
2. One child = one worktree = one branch = one PR.
3. Autonomy stops at **CI green + mergeable PR** — not merge.
4. **No auto-merge** or force-push to main/master.
5. Harness uses portable bash / `git` / `gh` only — no proprietary IDE APIs.
6. Do **not** invent product decisions to unblock; surface **BLOCKED**.

Intake-only: no issues, PRD, plans, branches, or code before user **GO** on the
epic breakdown. English for GitHub / foam artifacts; match user language in chat.

## Pipeline

```
Resolve context (repo, foam, board, agent CLI)
  → Ensure artifacts (mode-dependent; GO gate for intake)
  → Frontier gating (hold children with an open `Blocked by:` issue)
  → Ownership check (non-overlapping paths when parallel)
  → Provision worktrees under .orch/worktrees/
  → Dispatch workers (job pool, cap ORCH_MAX_PARALLEL=3, queue overflow)
  → Live graph in `.orch/graph/current.json` (skill `cda-graph`; cdamux Orch graph tab)
  → Per worker: graph nodes implement → verify → (optional) fix → grounding verifier
  → Integrator synthesize when 2+ issues (OpenClaw-style fan-in)
  → Per worker: systematic review → open/update PR
  → CI loop (gh pr checks) until green or BLOCKED
  → Aggregate ledger → human report (mergeable PRs; do not merge)
```

Verify oracle (order): project commands from `AGENTS.md` / `nix develop` →
systematic review (critical/important fixed or BLOCKED) → remote CI green.
Graph motion budget: `ORCH_MAX_ITER=10` per worker (separate implement/verify nodes, not one fat loop).
`ORCH_GRAPH=0` disables graph file; `ORCH_GRAPH_VERIFY=1` enables foam-grounded verifier agent.

### Optional design loop (`CDA_DESIGN_LOOP`)

Default: workers do **not** run `cda-design-loop`.
Opt-in when UI files change and a project brief exists, if any of:

- env `CDA_DESIGN_LOOP=1`
- `--design-loop` on orch / dispatch (when wired)
- `AGENTS.md` Project-specific notes contains `design-loop: auto`

UI path heuristic: `*.vue`, `*.tsx`, `*.jsx`, `*.css`, `*.scss`, or paths under `pages/`, `app/`, `components/`, `layouts/`.

## When to use the harness

Prefer `scripts/orch.sh` when a supported agent CLI is on `PATH` (Cursor agent,
`claude`, `opencode`, or `ORCH_AGENT` / `--agent` override):

```bash
SKILL=~/.cursor/skills/cda-dev   # or claude/opencode install path
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
bash "$SKILL/scripts/orch.sh" --mode pr-fix --pr 88 --sandbox
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --dry-run
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 42 \
  --graph-plan plans/graph/issue-42.json
bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh plans/graph/issue-42.json
ORCH_GRAPH_VERIFY=1 bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12
```

Graph cookbook: [doc/graph-orchestration.md](../../../doc/graph-orchestration.md) and skill `cda-graph`.

### AFK auto (Ready poller)

Opt-in poller: discover Ready cards and run `ready-pickup` (not headless):

```bash
bash "$SKILL/scripts/orch-auto.sh" --repo /path/to/repo --once --dry-run
bash "$SKILL/scripts/orch-auto.sh" --repo /path/to/repo --once
```

Systemd user templates: `templates/systemd/` — copy, set `ORCH_REPO`, then
`systemctl --user enable --now cda-orch-auto.timer` (stop the timer to free the
machine). Prefer a dedicated host for AFK load. Units are **not** enabled by
`install.sh`.

### Headless one-task (no Kanban / worktree)

For a single prompt + verify loop against the current repo (no issue/PR):

```bash
bash "$SKILL/scripts/headless.sh" --prompt "fix the flaky auth test" --dry-run
bash "$SKILL/scripts/headless.sh" --prompt-file ./task.md --max-iter 5
```

Host readiness (does not install packages): `./scripts/doctor.sh` from the kit root.

On a TTY, workers show a live progress board (spinner + phase). Disable with
`--no-progress` or `ORCH_PROGRESS=0`.

Optional `--sandbox`: bubblewrap (Linux, off by default). If `bwrap` is missing,
exit non-zero — do not continue unsandboxed.

## Skill-only fallback

If the harness is missing, unusable, or no agent CLI is available, run the
**same pipeline** from this skill: host Task/subagents (or manual steps) for
worktrees, workers, verify, and CI watch. Use the same prompts, ledger template,
and hard rules. Master still does not edit product source.

## Worker contract (master injects)

Mode + issue#/PR#; plan path; epic PRD + ADRs; worktree path + branch; verify
hints; max-iterations; sandbox on/off. Workers must not touch other worktrees,
merge, skip verify, expand scope beyond approved plan/PRD, or invent product
decisions.

**BLOCKED:** ADR conflict, product ambiguity, missing secrets/access, ownership
collision, max iterations, CI red needing product/decision input.

## Integration

| Skill | Role |
| ----- | ---- |
| `cda-foam` | ADRs, index, post-merge import |
| `cda-prd` | Epic PRD (`intake`) |
| `cda-plan` | Plan per child issue |
| `cda-kanban` | Issues, board, claim protocol |
| `cda-commit` | Conventional Commits on worker branches |
| `cda-review` | Systematic review before / with PR |
| `cda-graph` | Graph orchestration model, foam export, OpenClaw mapping |
| `cda-doc-hygiene` | Layout verify; archive closed plans before long-running orch |
| Host plugins | Compose `loop-on-ci`, Bugbot, security review — do not replace |

## Out of scope (v1)

- Auto-merge or babysitting merge after CI green without explicit human action
- Full per-worktree runtime isolation (DB/Docker) — warn only; see design spec
- ADK eval harness, UI dashboard, Windows sandbox

## Links

| Resource | Path |
| -------- | ---- |
| Operator docs | [doc/orchestration.md](../../../doc/orchestration.md), [doc/architecture.md](../../../doc/architecture.md) |
| Conventions | [conventions.md](conventions.md) |
| Intake / worker / reviewer / pr-fix | [prompts/](prompts/) |
| Ledger | [templates/run-ledger.md](templates/run-ledger.md) |
| Harness | [scripts/orch.sh](scripts/orch.sh), [scripts/orch-auto.sh](scripts/orch-auto.sh) |
| Systemd templates | [templates/systemd/](templates/systemd/) |
| ADR-002 | [foam/decisions/ADR-002-orch-auto-and-fast-lane.md](../../../foam/decisions/ADR-002-orch-auto-and-fast-lane.md) |
