---
name: dev-orchestrator
description: >-
  Unified meta-orchestrator: intake, Ready pickup, or PR-fix through foam/kanban,
  git worktrees, verify loops, and CI-green PRs (no auto-merge). Portable across
  Cursor, Claude Code, and opencode via orch.sh harness or skill-only fallback.
  Use for multi-subject batches, parallel features, or unblocking red CI.
---

# Dev orchestrator

Announce: **"Using dev-orchestrator skill."**

## Role

**Glue master** — orchestrates foam, Kanban, worktrees, workers, verify, and CI.
Never edits product application source; workers do. Holds ledger + ids + paths,
not full worker diffs.

| Phase | Owns | Delegates to |
| ----- | ---- | ------------ |
| Resolve / artifacts | Mode-dependent bootstrap | `foam-project-memory`, `foam-prd`, `foam-plan`, `github-kanban-orchestrator` |
| Dispatch | Parallel workers in worktrees | [prompts/worker.md](prompts/worker.md), [prompts/reviewer.md](prompts/reviewer.md) |
| Verify loop | Local verify + review + CI watch | `scripts/orch.sh`, `code-reviewer`, project `AGENTS.md` |
| Aggregate | Ledger + human report | [templates/run-ledger.md](templates/run-ledger.md) |

Details: [conventions.md](conventions.md). Design:
[docs/superpowers/specs/2026-10-02-dev-orchestrator-design.md](../../../docs/superpowers/specs/2026-10-02-dev-orchestrator-design.md).

## Modes

| Mode | Trigger | Human GO | Bootstrap | Exit |
| ---- | ------- | -------- | --------- | ---- |
| `intake` | Multi-subject chat list | Yes (epic breakdown) | Epic issue + approved PRD + N Ready children + N plans | PRs CI-green |
| `ready-pickup` | Ready Kanban cards with PRD/plan | No | None (validate artifacts exist) | PRs CI-green |
| `pr-fix` | PR URL/number | No | None | Same PR CI-green |

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
  → Ownership check (non-overlapping paths when parallel)
  → Provision worktrees under .orch/worktrees/
  → Dispatch workers (job pool, cap ORCH_MAX_PARALLEL=3, queue overflow)
  → Per worker: implement → local verify → systematic review → open/update PR
  → CI loop (gh pr checks) until green or BLOCKED
  → Aggregate ledger → human report (mergeable PRs; do not merge)
```

Verify oracle (order): project commands from `AGENTS.md` / `nix develop` →
systematic review (critical/important fixed or BLOCKED) → remote CI green.
Loop budget: `ORCH_MAX_ITER=10` per worker; fresh agent context per iteration.

## When to use the harness

Prefer `scripts/orch.sh` when a supported agent CLI is on `PATH` (Cursor agent,
`claude`, `opencode`, or `ORCH_AGENT` / `--agent` override):

```bash
SKILL=~/.cursor/skills/dev-orchestrator   # or claude/opencode install path
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
bash "$SKILL/scripts/orch.sh" --mode pr-fix --pr 88 --sandbox
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --dry-run
```

Optional `--sandbox`: bubblewrap (Linux, off by default). If `bwrap` is missing,
warn and continue without sandbox.

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
| `foam-project-memory` | ADRs, index, post-merge import |
| `foam-prd` | Epic PRD (`intake`) |
| `foam-plan` | Plan per child issue |
| `github-kanban-orchestrator` | Issues, board, claim protocol |
| `git-commit` | Conventional Commits on worker branches |
| `code-reviewer` | Systematic review before / with PR |
| Host plugins | Compose `loop-on-ci`, Bugbot, security review — do not replace |

## Out of scope (v1)

- Auto-merge or babysitting merge after CI green without explicit human action
- Full per-worktree runtime isolation (DB/Docker) — warn only; see design spec
- ADK eval harness, UI dashboard, Windows sandbox
- Replacing `foam-batch-orchestrator` behavior in this file — alias points here
  with `mode=intake`

## Links

| Resource | Path |
| -------- | ---- |
| Conventions | [conventions.md](conventions.md) |
| Intake / worker / reviewer / pr-fix | [prompts/](prompts/) |
| Ledger | [templates/run-ledger.md](templates/run-ledger.md) |
| Harness | [scripts/orch.sh](scripts/orch.sh) |
