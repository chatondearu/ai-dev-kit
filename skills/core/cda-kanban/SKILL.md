---
name: cda-kanban
description: >-
  Orchestrates the repo foam/board kanban (ADR-016). Ready listing and status
  moves use foam cards, not GitHub Projects. GitHub Issues are optional
  (`github_issue` on a card) for comments and `Closes`. Use when listing Ready,
  claiming work, moving cards, or linking an opt-in GitHub issue.
---

# Foam kanban orchestrator

## Scope

Governs **execution status** for the current repository. It does not replace
code skills (`cda-dev`, `review-and-ship`, `new-branch-and-pr`) — it wires them
to `foam/board/`.

This skill is **repo-agnostic**. Default path is git + markdown. `gh` is only
required for cards that opted into a GitHub issue.

## Prerequisites

1. Run from inside the target git repo (`git rev-parse --show-toplevel`)
2. `foam/board/` with one markdown file per card (see repo `foam/board/README.md`)
3. `gh` authenticated only when a card has `github_issue` (comments / `Closes`)

`git pull` (or fetch) before listing Ready so the worktree is not stale.

## Source of truth

| Layer | Role |
|-------|------|
| **`foam/board/`** | Status, priority, blockers (ADR-016). Glob `*.md`; skip `README.md` and `_*.md` |
| **GitHub Issues** | Optional. Set `github_issue` for discussion, `Closes #N`, external trackers |
| **GitHub Projects** | Not SoT. Do not call Projects in list-ready, orch-auto, or status moves |
| **PRs** | Implementation + review; `Closes #N` only when the card has `github_issue` |

Conventions (labels, branches, claim protocol): [conventions.md](conventions.md)
Foam helper: [scripts/foam-board.sh](scripts/foam-board.sh)
Opt-in GitHub helper (comments / events, not Ready): [scripts/gh-board.sh](scripts/gh-board.sh), [reference.md](reference.md)

## Kanban columns (Status)

Default flow:

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

Status lives on the card file (`status:` in frontmatter). Do not invent a second kanban.

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

Pipeline context: `cda-plan` § End-to-end feature pipeline.

1. `foam-board.sh list-ready` — **Ready** cards whose `blocked_by` is clear
2. Confirm no other agent claimed it (card still `ready`, no open PR on the expected branch)
3. `foam-board.sh claim <id>` — writes `in-progress`; comments on GitHub only if `github_issue` is set. Never `gh issue create`.
4. Commit the card file
5. Create branch: `<type>/<id>-<short-slug>` (see conventions.md)
6. Implement; commit with Conventional Commits; `Closes #N` only when `github_issue` is set
7. Open PR (use `new-branch-and-pr` or `review-and-ship`)
8. `foam-board.sh set-status <id> in-review` and commit the card
9. After merge → `foam-board.sh reconcile --pr` moves every in-progress/in-review card whose PR merged to `done` on a foam-only PR that self-merges (ADR-016 §3b) and closes its `github_issue` if still open. orch.sh runs it after its merges and at run end; run it by hand after a maintainer merge.

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

### E0 — List Ready (for orch-auto)

```bash
./scripts/foam-board.sh list-ready          # github_issue numbers or slugs; no Projects
./scripts/foam-board.sh list-ready --json
./scripts/foam-board.sh set-status <id> in-progress
./scripts/foam-board.sh claim <id>
```

Used by `cda-dev` `orch-auto.sh` (AFK poller). Filtering `triage:ready-for-human`
and open PRs (cards with `github_issue` only) is orch-auto’s job, not this helper.

`gh-board.sh list-ready` still exists for the opt-in GitHub-issue/Projects path
and its tests; it is **not** the default Ready oracle.

### E — Reconcile Backlog (API-efficient)

When cards sit in **Backlog** (or other non-Done columns) but the work already
landed on the integration branch, reconcile Status without per-item REST loops:

```bash
# Dry-run (default): candidates + proposed moves
./scripts/gh-board.sh reconcile-backlog

# Apply moves: 1 GraphQL list + ceil(N/chunk) aliased mutations
./scripts/gh-board.sh reconcile-backlog --apply --chunk 40
```

Strong signals only (else `skip`): issue/PR `CLOSED`, PR `merged` in the list
payload, or `git log <base>` containing `Closes|Fixes|Resolves #N` (bare `#N` and `Refs #N` do not count). Status
field only — never force-closes issues. Details: [reference.md](reference.md).

### F — Listen & react to board events

The skill cannot run as a daemon. "Listening" = **polling the board activity feed**
and reacting. The helper aggregates, per board item and since a cursor, a
normalized event feed (comments, reviews, labels, assignment, PR merge/close, and
Status column changes via snapshot diff).

```bash
# One-off catch-up since the last run (cursor stored under .tmp/kanban/):
./scripts/gh-board.sh events

# Explicit window (no cursor write), machine-readable:
./scripts/gh-board.sh events --since 2026-06-01T00:00:00Z --json
```

Each event is `{ts, type, actor, number, title, url, detail}`. React per type:

| Event type | Trigger | Skill reaction |
|------------|---------|----------------|
| `comment` | New issue/PR comment | Read it. If it's an instruction/mention → act, then reply. If `Claimed for implementation` → respect the claim, don't double-pick. |
| `review_approved` | PR approved | Keep card in **In review**; it's ready to merge (hand off to `cda-dev` verify / CI-green loop). |
| `review_changes_requested` | Review requests changes | Move card → **In progress**; address feedback on the branch. |
| `review_commented` / `review_dismissed` | Review note / dismissed | Note it; act only if it contains a blocking request. |
| `pr_merged` | PR merged | Move linked issue card → **Done**; close the issue if not auto-closed. |
| `closed` | Issue/PR closed (no merge) | If the issue was closed without a merged PR → confirm intent; otherwise reconcile the card to **Done**/**Backlog**. |
| `labeled` / `unlabeled` | Label change | Map priority/size labels to the matching board field; if `blocked` added → annotate and keep out of **Ready**. |
| `assigned` / `unassigned` | Assignment change | Record ownership; avoid claiming items assigned to someone else. |
| `status_changed` | Card moved (manually or by another agent) | Reconcile: someone changed the column out-of-band — re-read state before acting. |

After reacting, always re-sync the board and report (see Output format).

#### Listen modes

| Mode | How | When |
|------|-----|------|
| **On-demand** | `gh-board.sh events` once | At the start of an orchestration turn (catch up). |
| **Loop (agent-native)** | `/loop 5m` running `gh-board.sh events` + reactions | Hands-on session where the agent reacts each cycle. |
| **Background watch** | `gh-board.sh watch --interval 60` in a background terminal; read the terminal file for new lines | Passive monitoring inside one session. |
| **Push (GitHub Actions)** | Drop the workflow template (see reference.md) into the repo | True event-driven automation across sessions. Needs a `project`-scoped token + runner. |

Add `.tmp/` to the repo's `.gitignore` — the cursor and Status snapshot live in `.tmp/kanban/`.

## Issue body template

> Always in English (see Guardrails).

```markdown
## Goal
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

```bash
./scripts/foam-board.sh set-status <id> in-progress   # also: ready | in-review | done | backlog
# then: git add foam/board/<file> && git commit

./scripts/foam-board.sh reconcile          # report cards whose PR merged (read-only)
./scripts/foam-board.sh reconcile --write  # mark them done in this working tree
./scripts/foam-board.sh reconcile --pr     # branch off origin/<default>, PR, ADR-016 self-merge
```

Do not write GitHub Projects by default. `gh-board env` / `gh project item-edit`
stay in [reference.md](reference.md) for an optional mirror only.

## Bootstrap a board (no project yet)

```bash
gh project create --owner "$OWNER" --title "<Repo> Board"
gh project link <number> --owner "$OWNER" --repo "$OWNER/$REPO"
```

Then add `Status`, `Priority`, `Size` single-select fields in the GitHub UI (or via `gh project field-create`), and re-run `gh-board env`.

## Integration with other skills

| Phase | Skill |
|-------|-------|
| Chat multi-task → epic + parallel workers | `cda-dev` |
| Plan decomposition | `cda-plan` |
| Execute plan tasks | `cda-dev` |
| Open / update PR | `new-branch-and-pr`, `review-and-ship` |
| PR merge-ready | `cda-dev`, `loop-on-ci`, `fix-ci` |
| Split large work | `split-to-prs` |

### Batch from chat → cda-dev

When the user gives several subjects in chat and wants foam-aware parallel
delivery to open PRs, do **not** invent an ad-hoc multi-claim loop here. Hand
off to **`cda-dev`** (intake → GO → epic issue + PRD → N Ready
children → parallel claim/branch/PR). This skill still owns board IDs, claim
protocol, and status transitions used by those workers.

Always sync board status when those skills change PR/issue state.

## Guardrails

- **Never** force-push `main` / `master`
- **Never** close issues without a merged PR or explicit user request
- **Never** create duplicate issues — search first: `gh issue list --search "keywords"`
- **Never** batch unrelated changes in one PR
- **Never** hardcode another repo's project/field IDs — always re-resolve
- Use Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`)
- **Always write issues and PRs in English** (titles AND bodies), as well as commit
  messages and code comments. Match the user's language only in chat replies, never
  in the artifacts created on GitHub. Translate existing non-English issues/PRs when
  you touch them.

## Integration with cda-foam

| Kanban layer | Foam layer (`cda-foam`) |
|--------------|-------------------------------------|
| Card `foam/board/*.md` | Execution status (this skill) |
| Optional `github_issue` | `prd/PRD-NNN-*.md` source |
| Epic / Done | `foam/features/shipped.md` (or `import-kanban.sh`) |
| Implementation | `plans/PLAN-NNN-*.md` |
| Architecture choice | `foam/decisions/ADR-NNN-*.md` |

One-shot map of existing GH issues → cards: `cda-foam` `import-board.sh` (not a loop).
After merge: `foam-board.sh reconcile --pr` moves the card to `done`; add ADR when a new technical decision was made.

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
