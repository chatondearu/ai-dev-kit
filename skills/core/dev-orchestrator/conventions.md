# Dev orchestrator — conventions

## Modes

| Mode | Requires | Creates |
| ---- | -------- | ------- |
| `intake` | User GO on epic breakdown | Epic issue, approved PRD, Ready children, plans |
| `ready-pickup` | Ready issues + existing PRD/plan | Nothing (validate only) |
| `pr-fix` | Open PR number/URL | Nothing (validate only) |

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

Master keeps a session ledger (chat or repo file under `.orch/` if useful):

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
| `ci` | Waiting on remote checks (`gh pr checks`) |
| `done` | PR open, checks green, card In review |
| `blocked` | Needs human; see reason |

## Exit criteria

A worker is **done** only when its PR is open **and** `gh pr checks` reports green
(or the run is **blocked** with a documented reason). **Never merge** automatically.

## Worktrees

Path pattern: `.orch/worktrees/<issue_or_pr>-<slug>/`

Add `.orch/` to `.gitignore` in target repos so worktrees and ledger artifacts stay local.

## Environment defaults

| Variable | Default | Meaning |
| -------- | ------- | ------- |
| `ORCH_MAX_PARALLEL` | `3` | Max concurrent workers |
| `ORCH_MAX_ITER` | `10` | Max verify/fix iterations per worker |
| `ORCH_BASE_PORT` | `3900` | Base port for local services (increment per worker) |

## Parallelism

- Default concurrent cap: **3** (`ORCH_MAX_PARALLEL`).
- When `ORCH_MAX_PARALLEL` **> 1**, **git worktrees are required** (one worktree per active worker).
- If the breakdown marks hard deps (B needs A), still create both issues; either
  sequence those two workers or mark the dependent **blocked** until A's PR exists.
- With `ORCH_MAX_PARALLEL=1`, a single clone with one branch is acceptable.

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
