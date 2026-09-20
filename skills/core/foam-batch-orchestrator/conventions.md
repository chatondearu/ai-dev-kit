# Foam batch orchestrator — conventions

## Foam vs Kanban mapping (batch mode)

| Layer | Artifact | Rule |
| ----- | -------- | ---- |
| Epic | Parent GitHub issue | Umbrella; lists children; not claimed by workers |
| Product spec | **One** `prd/PRD-<epic#>-slug.md` | Status `approved` after user GO |
| Work units | Child issues | Status **Ready** before dispatch |
| Implementation | `plans/PLAN-<child#>-slug.md` per child | Cite epic PRD + ADRs in constraints |
| Execution | Board Status on **children** | Ready → In progress → In review → Done |

Unlike a solo feature (1 issue ↔ 1 PRD), batch mode keeps **one PRD on the epic**.
Children do not get separate PRDs unless the user later splits the epic.

## Parent / child issues

### Parent (epic) body (minimum)

```markdown
## Epic
<one paragraph>

## Children
- #N — <title>
- #M — <title>

## PRD
`prd/PRD-<epic#>-slug.md`

## Non-goals
- …
```

### Child body (minimum)

```markdown
## Parent
#<epic#>

## Goal
…

## Acceptance criteria
- [ ] …

## Plan
`plans/PLAN-<child#>-slug.md`

## PRD
`prd/PRD-<epic#>-slug.md` (epic)
```

Link children from the parent and parent from each child. Prefer a board
"Parent issue" field when the project has one; otherwise body links are enough.

## Naming

Follow `github-kanban-orchestrator` conventions:

- Issue titles: `<type>(<scope>): <imperative short description>`
- Branches: `<type>/<child#>-<slug>`
- One child = one branch = one PR (`Closes #<child#>`)
- Never share a branch across workers

## Run ledger

Master keeps a session ledger (chat or repo file under `.tmp/batch/` if useful):

See [templates/run-ledger.md](templates/run-ledger.md).

Worker report shape (required):

```json
{
  "issue": 42,
  "pr_url": "https://github.com/org/repo/pull/99",
  "status": "done",
  "reason": null
}
```

`status`: `done` | `blocked`  
On `blocked`, `reason` is a short English sentence; `pr_url` may be null.

## Worker status (ledger)

| Status | Meaning |
| ------ | ------- |
| `pending` | Not dispatched yet |
| `running` | Worker claimed / implementing |
| `review` | Waiting on systematic review / fixes |
| `done` | PR open, card In review |
| `blocked` | Needs human; see reason |

## Parallelism

- Always dispatch independent children in parallel after bootstrap.
- If the breakdown marks hard deps (B needs A), still create both issues; either
  sequence those two workers or mark the dependent **blocked** until A's PR exists.
- Prefer git worktrees per child when available; otherwise separate branches on
  one clone with strict non-overlapping file ownership from the plans.

## Claim protocol

Unchanged from `github-kanban-orchestrator`:

1. Child must be **Ready**
2. Comment: `Claimed for implementation.`
3. Move → **In progress** before first commit
4. After PR → **In review**

Epic parent is not claimed for implementation.

## GO gate

User GO means:

- Epic breakdown accepted
- Epic PRD may be written as **`approved`**
- Children may be set **Ready** and workers may start

Revise + re-present the breakdown if the user rejects GO.
