# PLAN-405 — feat(skills): list-ready and orch from foam board, not GitHub Projects

- **Status**: draft
- **PRD**: [[../prd/PRD-403-foam-first-kanban|PRD-403]]
- **GitHub**: [#405](https://github.com/chatondearu/ai-dev-kit/issues/405)
- **Date**: 2026-11-19

## Summary

Point `cda-kanban`, `cda-foam`, `cda-dev` orch/orch-auto, and
`cda-orchestrator` at `foam/board/` for execution status. Ready listing must
not call GitHub Projects by default. `gh issue` only when a card has
`github_issue`.

Blocked by: #404 (ADR + card contract on main).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]] | Foam is SoT; glob cards; no Projects in the default Ready path; optional `github_issue` |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | orch-auto still polls Ready and calls `orch.sh --mode ready-pickup`; only the Ready *oracle* changes |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] | Global orch reads `$CDAMUX_HOME/board-index.json` (file from #407; this issue wires the skill) |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Foam board works with git + `gh` for opt-in issues; do not require cdamux for per-repo list-ready |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- list-ready from foam cards with `status: ready` (and not `blocked_by` open)
- orch.sh / orch-auto consume that list
- cda-foam: board layout in the skill; one-shot import/map helper from GH issues
- cda-orchestrator: refresh + read the global index at turn start (tolerate missing file)
- Status moves = git commits on the card file (Ready → In progress → In review → Done)
- Keep gh-board for opt-in GitHub issue comments / `Closes` only

### Out of scope

- Worker `gh pr merge` (#406)
- Implementing the index writer (#407)
- Rollout to other repos (#408)
- Fleet Foam browser (#265)

## Technical approach

Replace the Ready oracle first (one function both orch-auto and manual
list-ready call): parse frontmatter, filter `status: ready`, skip cards
whose `blocked_by` still refers to an open issue or a non-done card.
Claim protocol writes `status: in-progress` on that file. GitHub Projects
mutations drop out of the default path. If a card has `github_issue`,
existing `gh issue comment` / PR `Closes` stay.

`cda-orchestrator` documents: run index refresh if `cdamux` is on PATH,
then read `$CDAMUX_HOME/board-index.json`; if absent, fall back to
per-repo foam glob.

## Tasks

- [ ] list-ready returns Ready foam cards without calling Projects; a fixture repo with three cards (ready / backlog / blocked) matches expected numbers
- [ ] orch-auto dry-run uses that list; existing skip rules (`triage:ready-for-human`, open PR on the expected branch) still apply to cards with `github_issue`
- [ ] Claim / column moves update the card file status (and `updated`); they do not write Projects by default
- [ ] Cards with `github_issue` still get `gh issue` comment + `Closes`; cards without one never call `gh issue create`
- [ ] cda-foam skill + conventions describe `foam/board/` as SoT; one-shot map from `gh issue list` is documented as import, not a loop
- [ ] cda-orchestrator turn start: refresh index when possible, read it, do not crash when the file or a project foam is missing
- [ ] Tests for the parser and list-ready; `bash scripts/verify.sh`; PR `Closes #405`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Overlap with #406 on `cda-dev` | #406 is `Blocked by: #405`; do not add merge logic here |
| Index file not on main yet (#407) | Skill reads the path ADR-016 names; missing file is a fallback, not a hard error |
| Stale worktree lists a moved card | Document `git pull` / refresh before list-ready (index refresh is #407) |

## Test plan

Seams: list-ready stdout (issue numbers or slugs); orch-auto dry-run;
frontmatter parser on `_template.md` + fixtures; orchestrator skill text
names the index path.

- [ ] Fixture: ready card listed; backlog not; `blocked_by` on an open blocker not
- [ ] Existing gh-board tests still pass for the opt-in GitHub-issue path
- [ ] `bash scripts/verify.sh`

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR if a new decision was made
