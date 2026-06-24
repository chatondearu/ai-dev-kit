# Conventions — GitHub Kanban Orchestrator

Repo-agnostic conventions. All identifiers are resolved at runtime by
[`scripts/gh-board.sh`](scripts/gh-board.sh) — never hardcode them here.

## Board model (expected fields)

The skill works best when the linked Projects (v2) board has these
single-select fields. Option names are free; the helper maps them automatically.

| Field | Suggested options |
|-------|-------------------|
| `Status` | Backlog, Ready, In progress, In review, Done |
| `Priority` | P0, P1, P2 |
| `Size` | XS, S, M, L, XL |

`gh-board env` exports one variable per option, named `<FIELD>_<OPTION>`
(e.g. `STATUS_IN_PROGRESS`, `PRIORITY_P0`, `SIZE_M`) plus `<FIELD>_FIELD_ID`.

## Labels (recommended)

Create when bootstrapping a backlog (idempotent with `--force`):

| Label | Color | Purpose |
|-------|-------|---------|
| `type:feat` | `A2EEEF` | New feature |
| `type:fix` | `D73A4A` | Bug fix |
| `type:chore` | `5319E7` | Tooling, CI, deps |
| `type:docs` | `0075CA` | Documentation |
| `area:*` | project-specific | Code area / module |

Priority lives on the **project field**, not as a duplicate label.

```bash
gh label create "type:feat" --color "A2EEEF" --description "New feature" --force
```

## Milestones (phase-based)

Use short, ordered titles, e.g. `M1-foundation`, `M2-core`, `M3-release`.

```bash
gh api "repos/$OWNER/$REPO/milestones" -X POST \
  -f title="M1-foundation" -f description="Phase 1"
```

## Issue title format

```
<type>(<scope>): <imperative short description>
```

Examples:
- `feat(archive): add locale-aware filters`
- `fix(i18n): restore missing keys`
- `chore(ci): cache package store`

## Branch naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature | `feat/<issue#>-<slug>` | `feat/12-archive-filters` |
| Fix | `fix/<issue#>-<slug>` | `fix/8-i18n-keys` |
| Chore | `chore/<issue#>-<slug>` | `chore/3-ci-cache` |
| Docs | `docs/<issue#>-<slug>` | `docs/5-deploy-guide` |

## Agent claim protocol

1. Issue must be **Ready** on the board
2. Comment: `Claimed for implementation.`
3. Optional self-assign: `gh issue edit N --add-assignee @me`
4. Move to **In progress** before the first commit

## Definition of done (issue)

- Acceptance criteria in the issue body checked
- Project build / tests pass locally
- PR merged to the default branch
- Board card **Done**
- Issue closed (via `Closes #N` or manually)

## Multi-agent rules

| Rule | Detail |
|------|--------|
| Granularity | One issue per agent session |
| Collision | If same file area, sequentialize or split the issue |
| Parent/child | Large epics → parent issue + sub-issues on the board |
| Handoff | Comment with branch name, PR link, remaining checklist |

## Per-project overrides (optional)

If a repo needs fixed choices (specific project number when several are linked,
or a non-default board), record them in the repo itself — for example a
`.github/kanban.env` file — and pass them explicitly:

```bash
eval "$(scripts/gh-board.sh env --project-number 2)"
```

Do not store another repo's resolved IDs in this shared skill.
