# cda-explore conventions

## Labels

| Label | On | Meaning |
| ----- | -- | ------- |
| `explore:map` | the map issue | the canonical index for one effort |
| `explore:research` | ticket | AFK fact-finding (resolve via `deep-research`) |
| `explore:prototype` | ticket | HITL concrete artifact (resolve via `cda-prototype`) |
| `explore:grilling` | ticket | HITL conversation — the default |
| `explore:task` | ticket | manual work that unblocks a decision |

`explore:*` are the only labels a map and its tickets carry. **Never** apply a work label like `ready-for-agent` / Status=Ready to a decision ticket: these are decisions, not implementation work, and must not be picked up by `cda-dev`.

## Map ↔ ticket relationship

Each ticket is a **GitHub sub-issue** of the map (`gh` GraphQL `addSubIssue`, or the issue UI). If sub-issues are unavailable on the repo, fall back to a `Parent: #<map>` line in the ticket body and an `explore:map`-scoped task list in the map.

## Blocking edges and the frontier

Blocking is recorded as a body line on the blocked ticket:

```
Blocked by: #12, #15
```

- A ticket is **unblocked** when every issue in its `Blocked by:` line is **closed**.
- The **frontier** = open tickets that are unblocked **and** unassigned (unclaimed).
- A session **claims** a ticket by assigning it to the driver *before any work*; the assignee is the claim.

This is the same `Blocked by:` convention `cda-dev` reads to compute its dispatch frontier, so a decision that graduates into implementation work keeps one dependency language end to end. If the repo has GitHub's native issue-dependencies enabled, record the edge there too (it renders the frontier visually), but the `Blocked by:` line stays authoritative for tooling.

### Frontier query

```bash
# Open decision tickets, with assignee + body, to compute the frontier locally.
# NOTE: repeated --label is AND; a ticket has only one explore:<type>, so use a
# comma-OR search qualifier (label:a,b,c matches any) instead.
gh issue list --repo "$OWNER/$REPO" \
  --search "label:explore:grilling,explore:research,explore:prototype,explore:task state:open" \
  --json number,title,assignees,body
# a ticket is on the frontier when: assignees == [] AND every `Blocked by:` number is a closed issue
```

Resolve `OWNER/REPO` and board IDs with `cda-kanban`'s `scripts/gh-board.sh env` — never hardcode them.

## Wiring to foam and kanban

- **Decisions → `cda-foam`.** When a ticket locks a real architectural/product decision, record it as an ADR (`foam/decisions/ADR-NNN-*.md`) through `cda-foam`, and link the ADR from the map's *Decisions so far* line. The map gists; the ADR is the store.
- **Map → epic (`cda-kanban`).** If the effort is tracked on the board, create the map under (or linked to) the epic issue, and keep the map in `Backlog`/a dedicated column — a map is not pickable work.
- **Handoff → `cda-prd`.** When the frontier empties, the destination is clear: the recorded ADRs are the constraints `cda-prd` links, and the destination is the PRD's goal. Hand off; do not write the PRD from inside the map.

## One ticket per session

Never resolve more than one ticket per session (research tickets excepted — they run as parallel subagents). Resolving changes the fog, and the next choice should be made against the updated map, not a stale one. Concurrent sessions may work other frontier tickets; expect the tracker to change under you.
