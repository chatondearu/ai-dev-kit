# gh CLI reference — GitHub Kanban Orchestrator

Repo-agnostic recipes. Resolve identifiers once, then reuse the exported vars.

## 0. Resolve context (run first)

```bash
# From inside the target repo:
eval "$(scripts/gh-board.sh env)"
# Now available: OWNER REPO PROJECT_NUMBER PROJECT_ID
#   STATUS_FIELD_ID, STATUS_BACKLOG, STATUS_READY, STATUS_IN_PROGRESS,
#   STATUS_IN_REVIEW, STATUS_DONE
#   PRIORITY_FIELD_ID, PRIORITY_P0, PRIORITY_P1, PRIORITY_P2
#   SIZE_FIELD_ID, SIZE_XS … SIZE_XL
env | grep -E '^(OWNER|REPO|PROJECT_|STATUS_|PRIORITY_|SIZE_)' | sort
```

Override detection when needed:

```bash
eval "$(scripts/gh-board.sh env --project-number 2 --owner some-owner)"
```

## Inspect state

```bash
gh project view "$PROJECT_NUMBER" --owner "$OWNER"
gh project item-list "$PROJECT_NUMBER" --owner "$OWNER" --limit 100 --format json
gh issue list --repo "$OWNER/$REPO" --state open --limit 50
gh pr list --repo "$OWNER/$REPO" --state open
gh api "repos/$OWNER/$REPO/milestones" --jq '.[] | "#\(.number) \(.title) [\(.state)]"'
gh label list --repo "$OWNER/$REPO" --limit 100
```

## Create milestone

```bash
gh api "repos/$OWNER/$REPO/milestones" -X POST \
  -f title="M1-foundation" -f description="Phase 1"
```

## Create issue + add to board

```bash
gh issue create \
  --repo "$OWNER/$REPO" \
  --title "feat(scope): short description" \
  --body "$(cat <<'EOF'
## Objectif
…

## Scope
- [ ] …

## Definition of done
- [ ] Build passes
- [ ] CI green on PR
EOF
)" \
  --label "type:feat" \
  --milestone "M1-foundation"

# Then add the created issue to the board:
gh project item-add "$PROJECT_NUMBER" --owner "$OWNER" \
  --url "https://github.com/$OWNER/$REPO/issues/<N>"
```

`gh issue create --project "<Board title>"` also works but adding by URL after
creation is more reliable in scripts.

## Resolve a board item id from an issue number

```bash
ITEM_ID="$(scripts/gh-board.sh item-id <ISSUE_NUMBER>)"
```

## Update Status (move Kanban column)

```bash
gh project item-edit \
  --project-id "$PROJECT_ID" \
  --id "$ITEM_ID" \
  --field-id "$STATUS_FIELD_ID" \
  --single-select-option-id "$STATUS_IN_PROGRESS"   # or $STATUS_IN_REVIEW / $STATUS_DONE
```

## Update Priority / Size

```bash
gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM_ID" \
  --field-id "$PRIORITY_FIELD_ID" --single-select-option-id "$PRIORITY_P1"

gh project item-edit --project-id "$PROJECT_ID" --id "$ITEM_ID" \
  --field-id "$SIZE_FIELD_ID" --single-select-option-id "$SIZE_M"
```

## Branch + PR (linked to issue)

```bash
ISSUE=12
git fetch origin
git checkout -b "feat/${ISSUE}-short-slug" origin/HEAD
# … work …
git push -u origin HEAD

gh pr create \
  --repo "$OWNER/$REPO" \
  --title "feat(scope): short description" \
  --body "$(cat <<EOF
## Summary
- …

## Issue
Closes #${ISSUE}

## Test plan
- [ ] …
EOF
)"
```

## Close / reopen issue

```bash
gh issue close 12 --comment "Merged via PR #N"
gh issue reopen 12
```

## Listen to board events

```bash
# Catch up since the last run (cursor + Status snapshot stored in .tmp/kanban/):
scripts/gh-board.sh events

# Explicit window, machine-readable (no cursor write):
scripts/gh-board.sh events --since 2026-06-01T00:00:00Z --json

# Limit how many board items are scanned (default 100):
scripts/gh-board.sh events --limit 50

# Passive monitoring: poll every 60s (run in a background terminal):
scripts/gh-board.sh watch --interval 60
```

Event object: `{ts, type, actor, number, title, url, detail}`.
Types: `comment`, `review_approved`, `review_changes_requested`,
`review_commented`, `review_dismissed`, `pr_merged`, `closed`, `labeled`,
`unlabeled`, `assigned`, `unassigned`, `status_changed`.

- Activity (comments/reviews/labels/assign/merge/close) comes from each board
  item's **timeline** (`repos/{owner}/{repo}/issues/{n}/timeline`).
- `status_changed` is derived from a **snapshot diff** of each item's Status
  (Projects v2 exposes no field-history API). First run only seeds the snapshot.
- The cursor advances to the run start time, so the next `events` only returns
  newer activity. Use `--since` for one-off windows without touching the cursor.

Reactions per type: see SKILL.md → "E — Listen & react to board events".

## Push automation (GitHub Actions)

For true event-driven automation across sessions, copy
[`templates/kanban-events.yml`](templates/kanban-events.yml) into the target repo
as `.github/workflows/kanban-events.yml`.

Out of the box it reacts to `pull_request` (merged) by moving the linked issue's
card to **Done**, and to `issue_comment` starting with `/board` by leaving an
acknowledgement. The template is **self-contained**: it resolves the project,
Status field and `Done` option at runtime via `gh` + GraphQL (same logic as
`gh-board.sh`), so no helper file needs to be vendored into the repo.

Requirement: a **`project`-scoped token** stored as the `GH_PROJECT_TOKEN`
secret. The default `GITHUB_TOKEN` cannot write Projects (v2) on most
accounts/orgs.

## Bulk bootstrap labels

```bash
for spec in \
  "type:feat:A2EEEF:New feature" \
  "type:fix:D73A4A:Bug fix" \
  "type:chore:5319E7:Tooling CI deps" \
  "type:docs:0075CA:Documentation"
do
  IFS=: read -r name color desc <<< "$spec"
  gh label create "$name" --color "$color" --description "$desc" --force
done
```

## Bootstrap a board (none linked yet)

```bash
gh project create --owner "$OWNER" --title "$REPO Board"
gh project link <number> --owner "$OWNER" --repo "$OWNER/$REPO"
# Add Status/Priority/Size single-select fields (UI or field-create), then:
eval "$(scripts/gh-board.sh env)"
```

## Troubleshooting

| Error | Fix |
|-------|-----|
| `project` scope missing | `gh auth refresh -s project` |
| multiple linked projects | pass `--project-number N` to `gh-board.sh env` |
| unknown field/option | re-run `gh-board.sh env`; the board's options may have changed |
| no project linked | create + link a board (see above) |
| item not on board | `gh project item-add "$PROJECT_NUMBER" --owner "$OWNER" --url <issue-url>` |
| duplicate issue | `gh issue list --search "in:title keywords"` before creating |
