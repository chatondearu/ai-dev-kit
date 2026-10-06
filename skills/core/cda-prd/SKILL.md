---
name: cda-prd
description: >-
  Draft or update Product Requirements Documents (PRD) under prd/, linked to a
  GitHub Kanban issue and foam ADRs. Use when scoping a feature, moving an issue
  to Ready, writing product specs before implementation, or when the user asks
  for a PRD.
---

# Foam PRD authoring

Announce at start: **"Using cda-prd skill."**

## Role

Turn a **Kanban issue** into a versioned **PRD** (`prd/PRD-NNN-slug.md`) wired to
**foam** (ADRs, product notes) and **cda-kanban** (board status).

| Layer | This skill | Other skills |
| ----- | ---------- | ------------ |
| Issue on board | source material | `cda-kanban` |
| PRD file | **creates / updates** | — |
| ADR constraints | links + confront | `cda-foam` |
| Implementation plan | pointer only | `cda-plan` |

## Before writing

1. Read `AGENTS.md` and `foam/index.md`.
2. Search existing PRDs: `ls prd/PRD-*` and `grep` keywords.
3. Search foam ADRs: `foam/decisions/`.
4. Search GitHub overlap:
   ```bash
   gh issue list --state all --search "keywords" --limit 20
   gh pr list --state all --search "keywords" --limit 20
   ```
5. If an accepted ADR contradicts the feature — cite it and ask to supersede.

## Create PRD from issue

```bash
SKILL=~/.cursor/skills/cda-prd

bash "$SKILL/scripts/new-prd.sh" 42
bash "$SKILL/scripts/new-prd.sh" 42 my-feature-slug
bash "$SKILL/scripts/new-prd.sh" --status approved 42
bash "$SKILL/scripts/new-prd.sh" --dry-run 42
```

Script copies `cda-foam` template, pre-fills GitHub link and Goal from
the issue body, updates `foam/index.md` PRD table when present.

## Manual workflow

1. **Kanban**: ensure issue exists (`cda-kanban` workflow A).
2. **PRD**: run `new-prd.sh <issue#>` or copy `prd/template.md` → `prd/PRD-NNN-slug.md`.
3. Fill: problem, goals, non-goals, requirements, related ADRs (wikilinks).
4. Set status: `draft` → `review` → `approved`.
5. **Board**: move card to **Ready** only when PRD is `approved` (or explicitly waived for trivial scope).
6. Comment on issue with PRD path when ready for implementation.

## PRD quality bar

- Every requirement traceable to issue scope or user scenario.
- **Non-goals** explicit (prevents scope creep).
- **Related project memory** lists real ADR wikilinks — no invented decisions.
- Open questions listed before approval.
- English for all PRD content.

## After approval

- Hand off to **`cda-plan`** for `plans/PLAN-NNN-slug.md`.
- On ship: set status `shipped`; run `cda-foam/scripts/import-kanban.sh`.

## Status values

`draft` | `review` | `approved` | `shipped` | `partial` | `backlog`

See [conventions.md](conventions.md) and `cda-foam/conventions.md`.
