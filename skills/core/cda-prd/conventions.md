# Foam PRD — conventions

## Naming

- File: `prd/PRD-NNN-short-slug.md` — **NNN = GitHub issue number**
- Title: `# PRD-NNN — Feature title`

## Kanban linkage

| PRD status | Suggested board Status |
| ---------- | ---------------------- |
| `draft`, `backlog` | Backlog |
| `review` | Backlog or Ready (team choice) |
| `approved` | **Ready** (pickable) |
| `shipped` | **Done** (via merged PR) |

## Issue → PRD mapping

One primary PRD per issue for non-trivial features. Epics may spawn child issues
each with their own PRD; link parent in PRD **Related project memory**.

**Batch orchestrator exception** (`cda-dev`): one PRD on the
**epic** issue only; children get plans (`plans/PLAN-<child#>-…`) that cite the
epic PRD — not separate child PRDs.

## When to skip a PRD

Trivial fixes (typos, one-line bug) — issue body is enough. Still search ADRs and
open PRs before coding.
