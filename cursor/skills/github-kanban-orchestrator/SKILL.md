---
name: github-kanban-orchestrator
description: >-
  Orchestrates any GitHub repository's backlog via milestones, issues, PRs, and a
  GitHub Projects (v2) Kanban board. Auto-detects the repo and project at runtime
  (no hardcoded IDs). Use when planning work, creating or triaging issues, moving
  cards across columns, coordinating one or several agents on tasks, or linking
  branches and PRs to board items.
---

# GitHub Kanban Orchestrator

Announce at start: **"Using github-kanban-orchestrator skill."**

## Scope

Governs **project management on GitHub** for the current repository. It does not replace code skills (`review-and-ship`, `babysit`, `new-branch-and-pr`) — it wires them to a Kanban board.

This skill is **repo-agnostic**: it resolves the owner, repo, project number, and field/option IDs at runtime. Nothing is hardcoded.

## Prerequisites

1. `gh` authenticated: `gh auth status`
2. Token has `project` scope. If missing: `gh auth refresh -s project`
3. Run from inside the target git repo (commands use `git rev-parse --show-toplevel`)
4. A GitHub Projects (v2) board linked to the repo or owner. If none exists, see "Bootstrap a board".

## Resolve context first (always)

Before any board action, resolve identifiers with the helper:

```bash
source "$(dirname "$0")/scripts/gh-board.sh"   # or run subcommands directly
gh-board env        # prints OWNER, REPO, PROJECT_NUMBER, PROJECT_ID, field + option IDs
```

The helper auto-detects:
- `OWNER/REPO` via `gh repo view --json owner,name`
- The linked project (first project linked to the repo, or prompts if several)
- `Status`, `Priority`, `Size` field IDs and their option IDs via `gh project field-list`

If detection is ambiguous (multiple projects), pass an override:

```bash
gh-board env --project-number 2 --owner some-owner
```

Never paste IDs from another repo. Re-resolve per repository.

## Source of truth

| Layer | Role |
|-------|------|
| **GitHub Project** (Kanban board) | Status, priority, size, dates |
| **Issues** | Scoped work units, acceptance criteria, `Closes #N` linkage |
| **Milestones** | Phase / release grouping |
| **PRs** | Implementation + review; auto-linked when referencing issues |

Conventions (labels, branches, claim protocol): [conventions.md](conventions.md)
`gh` command recipes + the discovery helper: [reference.md](reference.md)

## Kanban columns (Status)

Default flow (adapt to the board's actual options, discovered at runtime):

```
Backlog → Ready → In progress → In review → Done
```

| Status | Meaning | Who moves |
|--------|---------|-----------|
| **Backlog** | Idea captured, not ready | Planner / human |
| **Ready** | Scoped, can be picked up | Planner |
| **In progress** | Branch open, active work | Implementing agent |
| **In review** | PR open, awaiting merge | Implementing agent after PR |
| **Done** | Merged + verified | Agent after merge (or human) |

If the board uses different option names, map intent to the closest existing option rather than inventing new ones.

## Core workflows

### A — Bootstrap backlog (new milestone / phase)

```
- [ ] gh-board env (resolve IDs)
- [ ] List existing milestones, labels, board items (avoid duplicates)
- [ ] Create milestone if missing
- [ ] Create labels if missing (see conventions.md)
- [ ] Create issues with body template (below)
- [ ] Add each issue to the board
- [ ] Set Priority + Size on board items
- [ ] Leave Status = Backlog (or Ready if fully scoped)
- [ ] Post summary table to user
```

### B — Pick up work (single agent)

1. `gh project item-list` — find **Ready** items without an active PR
2. Confirm no other agent claimed it (no open PR, no "Claimed" comment in last 24h)
3. Comment on issue: `Claimed for implementation.`
4. Move card → **In progress**
5. Create branch: `<type>/<issue#>-<short-slug>` (see conventions.md)
6. Implement; commit with Conventional Commits; reference `Refs #N` or `Closes #N`
7. Open PR (use `new-branch-and-pr` or `review-and-ship`)
8. Move card → **In review**
9. After merge + CI green → move card → **Done**, close issue if not auto-closed

### C — Multi-agent coordination

- **One issue = one branch = one PR.** Never share a branch across agents.
- Before claiming: check board + `gh pr list` for overlapping scope.
- Prefer **Ready** items with highest Priority (P0 first).
- If two agents could conflict: split into sub-issues, link via "Parent issue" on the board.
- Subagent dispatch: parent agent owns board updates; subagents report issue # only.

### D — Triage / reprioritize

1. Read open issues + board state
2. Adjust Priority / Size fields on project items
3. Move cards between Backlog ↔ Ready based on clarity
4. Comment on issue with rationale (one short paragraph max)

## Issue body template

```markdown
## Objectif
[One sentence]

## Scope
- [ ] …

## Out of scope
- …

## Definition of done
- [ ] Code + tests / build pass
- [ ] CI green on PR
- [ ] Docs updated if behavior changed

## Agent notes
- Milestone: …
- Priority: P0 | P1 | P2
- Size: XS | S | M | L | XL
```

## PR body template

```markdown
## Summary
- …

## Issue
Closes #N

## Test plan
- [ ] …
```

## Status transitions (quick commands)

After `gh-board env` exported the IDs and you resolved the item ID:

```bash
gh project item-edit \
  --project-id "$PROJECT_ID" \
  --id "$ITEM_ID" \
  --field-id "$STATUS_FIELD_ID" \
  --single-select-option-id "$STATUS_IN_PROGRESS"
```

All `$*_FIELD_ID` and `$STATUS_*` / `$PRIORITY_*` / `$SIZE_*` come from `gh-board env`. See [reference.md](reference.md).

## Bootstrap a board (no project yet)

```bash
gh project create --owner "$OWNER" --title "<Repo> Board"
gh project link <number> --owner "$OWNER" --repo "$OWNER/$REPO"
```

Then add `Status`, `Priority`, `Size` single-select fields in the GitHub UI (or via `gh project field-create`), and re-run `gh-board env`.

## Integration with other skills

| Phase | Skill |
|-------|-------|
| Plan decomposition | `writing-plans` |
| Execute plan tasks | `executing-plans`, `subagent-driven-development` |
| Open / update PR | `new-branch-and-pr`, `review-and-ship` |
| PR merge-ready | `babysit`, `loop-on-ci`, `fix-ci` |
| Split large work | `split-to-prs` |

Always sync board status when those skills change PR/issue state.

## Guardrails

- **Never** force-push `main` / `master`
- **Never** close issues without a merged PR or explicit user request
- **Never** create duplicate issues — search first: `gh issue list --search "keywords"`
- **Never** batch unrelated changes in one PR
- **Never** hardcode another repo's project/field IDs — always re-resolve
- Use Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`)
- Match user language for communication; English for issue/PR bodies and code comments

## Output format

After each orchestration action, report:

```markdown
### Board update
| Issue/PR | Status | Priority | Agent action |
|----------|--------|----------|--------------|
| #N …     | Ready → In progress | P1 | Claimed, branch created |

### Next
- [ ] …
```
