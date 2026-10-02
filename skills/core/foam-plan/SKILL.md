---
name: foam-plan
description: >-
  Create or update implementation plans under plans/ from an approved PRD and
  GitHub Kanban issue. Links ADR constraints from foam, produces a task checklist
  before coding. Use when a PRD is approved, picking up Ready work, or when the
  user asks for an implementation plan.
---

# Foam plan authoring

Announce at start: **"Using foam-plan skill."**

## Role

Turn an **approved PRD** + **Kanban issue** into an implementation **plan**
(`plans/PLAN-NNN-slug.md`) with ADR constraints and a verifiable task checklist.

| Step | Skill |
| ---- | ----- |
| Issue on board | `github-kanban-orchestrator` |
| PRD | `foam-prd` (required before plan) |
| Plan | **this skill** |
| Claim + branch + PR | `github-kanban-orchestrator` workflow B |
| Commit / review | `git-commit`, `code-reviewer` |

## Before writing

1. Confirm PRD exists and status is `approved` (or user waived for small scope).
2. Read the PRD and linked ADRs in `foam/decisions/`.
3. Search open PRs for the same issue: `gh pr list --search "#NNN"`.
4. Do not start coding until the plan checklist reflects real scope.

## Create plan from issue

```bash
SKILL=~/.cursor/skills/foam-plan

bash "$SKILL/scripts/new-plan.sh" 42
bash "$SKILL/scripts/new-plan.sh" --status in-progress 42
bash "$SKILL/scripts/new-plan.sh" --dry-run 42
```

Requires `prd/PRD-42-*.md` (create with `foam-prd/scripts/new-prd.sh` first).

## End-to-end feature pipeline

```
Issue (Backlog)
  → foam-prd: PRD draft → review → approved
  → Board: Ready
  → foam-plan: PLAN-NNN
  → github-kanban-orchestrator: claim → In progress → branch → PR
  → verify plan checklist + merge
  → PRD shipped, import-kanban.sh, ADR if new technical choice
```

### Batch from chat (several subjects)

> Note: `foam-batch-orchestrator` is an alias for `dev-orchestrator`.

When the user lists multiple subjects to analyze and implement in one run, use
**`foam-batch-orchestrator`** instead of repeating this pipeline manually:

```
Chat list → intake Q&A → GO on epic breakdown
  → epic issue + approved PRD
  → N child issues (Ready) + N plans (this skill per child)
  → parallel workers → PRs open
```

## Plan quality bar

- **Constraints** table lists accepted ADRs — violations need a new ADR.
- **Tasks** are checkboxes an agent can tick during implementation.
- **Test plan** matches project conventions (nix develop, CI, etc.).
- **Verification** section completed before merge.
- English for all plan content.

## During implementation

- Tick tasks in the plan as you go.
- Set plan status: `draft` → `in-progress` → `verify` → `done`.
- New architecture choice mid-flight → pause, record ADR, update plan constraints.

## After merge

- Plan status → `done`.
- PRD status → `shipped`.
- Run `foam-project-memory/scripts/import-kanban.sh`.

See [conventions.md](conventions.md).
