# Foam plan — conventions

## Naming

- File: `plans/PLAN-NNN-short-slug.md` — **NNN = GitHub issue number** (same as PRD)
- Title: `# PLAN-NNN — Feature title`
- Must link to `prd/PRD-NNN-*.md`

## Kanban linkage

| Plan status | Board Status |
| ----------- | ------------ |
| `draft` | Ready (PRD approved, plan being written) |
| `in-progress` | **In progress** (claimed) |
| `verify` | **In review** (PR open, final checks) |
| `done` | **Done** (merged) |

## Claim protocol

Only claim after plan exists (unless trivial issue). Follow
`cda-kanban` workflow B: comment `Claimed for implementation.`,
move card, create branch `<type>/<issue#>-<slug>`.

## PRD without plan

Allowed only for trivial scope (single-file fix, no ADR impact). Document waiver
in issue comment.
