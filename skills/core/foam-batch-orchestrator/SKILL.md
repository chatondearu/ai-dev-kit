---
name: foam-batch-orchestrator
description: >-
  Master agent for multi-task chat batches: intake Q&A until full understanding,
  one GO gate on an epic breakdown, then bootstrap foam (1 epic PRD + N child
  plans) and Kanban, and dispatch parallel workers (1 issue / 1 branch / 1 PR)
  with tests and systematic review until PRs are open. Use when the user lists
  several subjects to analyze, write, and implement in one run, or asks for a
  foam-aware batch / loop orchestrator.
---

# Foam batch orchestrator

Announce at start: **"Using foam-batch-orchestrator skill."**

## Role

**Glue master** — does not implement product code. Orchestrates existing skills:

| Phase | Owns | Delegates to |
| ----- | ---- | ------------ |
| Intake | Q&A + epic breakdown | [prompts/intake.md](prompts/intake.md) |
| Bootstrap | Epic + children artifacts | `foam-project-memory`, `foam-prd`, `foam-plan`, `github-kanban-orchestrator` |
| Dispatch | Parallel workers | [prompts/worker.md](prompts/worker.md), [prompts/reviewer.md](prompts/reviewer.md) |
| Aggregate | Ledger + final report | [templates/run-ledger.md](templates/run-ledger.md) |

v1 entry mode: **chat multi-task list** only. Modes for existing Ready cards or
pre-approved PRDs are out of scope.

## Hard rules

1. **No issues, PRD, plans, branches, or code before user GO** on the epic breakdown.
2. Master **never** edits application source; workers do.
3. After GO: **full auto until PRs are open** — stop only on **BLOCKED**.
4. **Always parallel** across children: one child issue = one branch = one PR.
5. English for all GitHub / foam artifacts; match user language only in chat.
6. Confront foam ADRs and open issues/PRs before bootstrap (mandatory workflow).

## Pipeline overview

```
User chat (several subjects)
  → Phase 0: Intake Q&A → epic-breakdown table
  → GO gate (human)
  → Phase 1: Epic issue + approved PRD → N child issues (Ready) + N plans
  → Phase 2: Dispatch N workers in parallel
  → Phase 3: Aggregate ledger → PR URL summary
```

See [conventions.md](conventions.md) for parent/child, ledger, and worker status.

## Phase 0 — Intake

Follow [prompts/intake.md](prompts/intake.md).

- Ask until objective, constraints, target repo, non-goals, success criteria, and
  cross-subject dependencies are clear.
- Prefer one question at a time (multiple choice when useful).
- Produce a filled [templates/epic-breakdown.md](templates/epic-breakdown.md).
- Present the breakdown and ask explicitly for **GO** (or revisions).

Do not proceed to Phase 1 without an unambiguous GO.

## Phase 1 — Bootstrap (after GO)

1. Read `AGENTS.md` and `foam/index.md` if present.
2. Search foam ADRs and GitHub for overlap; cite conflicts before creating work.
3. Unresolved ADR conflict → **BLOCKED** (ask user); do not invent superseding ADRs silently.
4. Create artifacts (reuse skill scripts / workflows — do not duplicate `gh` recipes):

| Artifact | How |
| -------- | --- |
| Parent (epic) issue | `github-kanban-orchestrator` — Backlog or Ready; body lists children |
| Child issues | One per breakdown row; link parent; Status **Ready**; labels/size/priority |
| Epic PRD | `foam-prd` → `prd/PRD-<epic#>-slug.md`, status **`approved`** (GO = approval) |
| Child plans | `foam-plan` → `plans/PLAN-<child#>-slug.md` each; constraints cite epic PRD + ADRs |

5. Update `foam/index.md` PRD/plan tables when the project uses them.
6. Snapshot child list into a run ledger ([templates/run-ledger.md](templates/run-ledger.md)).

Epic issue stays the product umbrella; **children** are the pickable work units.

## Phase 2 — Dispatch

For each Ready child, launch a **fresh** worker with minimal context:

- Child issue number + URL
- Plan path (`plans/PLAN-…`)
- Epic PRD path + issue number
- Relevant ADR paths / global constraints
- This skill’s worker + reviewer prompts

Worker contract: [prompts/worker.md](prompts/worker.md)  
Systematic review: [prompts/reviewer.md](prompts/reviewer.md) and/or `code-reviewer`

Master:

- Updates the run ledger as workers report
- Does not share one branch across workers
- Prefer isolated worktrees when the environment supports them
- Continues other workers if one is **BLOCKED**; surface BLOCKED to the user immediately

## Phase 3 — Aggregate

When every child is `done` or `blocked`:

```markdown
### Batch run complete
| Child | Plan | PR | Status | Notes |
|-------|------|-----|--------|-------|
| #N …  | plans/PLAN-… | url | done / blocked | … |

### Next (human)
- Review open PRs
- Merge when ready (merge/CI loop is out of scope for this skill)
```

Autonomy **ends at open PRs**. Do not merge or drive CI babysitting unless the
user explicitly asks another skill (`loop-on-ci`, etc.).

## BLOCKED (stop and ask the user)

Treat as BLOCKED and pause that worker (or the whole bootstrap if pre-dispatch):

| Condition | Example |
| --------- | ------- |
| ADR conflict | Accepted ADR contradicts the batch; user must supersede or change scope |
| Product ambiguity | Acceptance criteria still unclear after intake |
| Tests failing | Failures the worker cannot fix without product/decision input |
| Worktree / merge conflict | Parallel edits collide and need sequencing |
| Missing secrets / access | `gh` auth, tokens, private deps |
| Scope drift | Implementation would violate the approved epic PRD |

Do **not** invent product decisions to unblock. Report issue #, reason, options.

## Integration

| Skill | Role in this pipeline |
| ----- | --------------------- |
| `foam-project-memory` | ADRs, index, post-merge import |
| `foam-prd` | Single epic PRD |
| `foam-plan` | One plan per child |
| `github-kanban-orchestrator` | Issues, board, claim protocol |
| `code-reviewer` | Systematic review before PR |
| `git-commit` | Conventional Commits on worker branches |
| `project-agents-setup` | Lists this skill in `AGENTS.md` |

## Out of scope (v1)

- Entry from existing Ready-only batch or PRD-only pickup
- Auto-merge / CI babysit loops
- Cursor-only `agents/` definitions
- Long-running daemon `/loop` (session-driven skill only)
