---
name: cda-foam
description: >-
  Bootstrap and maintain a Foam-style project memory graph (ADR, PRD, plans) at
  repo-root foam/, wired to GitHub Kanban. Use when starting project memory,
  recording architecture decisions, drafting PRDs/plans, importing kanban issues
  into foam, or before product/architecture changes so the agent can challenge
  against accepted ADRs.
---

# Foam project memory

Announce at start: **"Using cda-foam skill."**

## What this is

A **versioned knowledge graph** in Markdown (wikilinks `[[note]]`) so agents and
humans share the same memory:

| Layer | Path | Role |
| ----- | ---- | ---- |
| **Foam graph** | `foam/` | MOC, ADRs, product notes, feature catalog |
| **PRD** | `prd/` | What / why per feature |
| **Plan** | `plans/` | How / when (checklist derived from PRD) |
| **Kanban** | GitHub Projects | Execution status (use `cda-kanban`) |

`foam/` is **agent decision memory**. User-facing docs may live elsewhere (`doc/`,
`README.md`) — do not duplicate; link from `foam/` when needed.

## Bootstrap a repository

Prefer **`cda-agents`** for a full bootstrap (`AGENTS.md` + optional foam):

```bash
bash ~/.cursor/skills/cda-agents/scripts/setup-agents.sh --type auto --init-foam
```

Or foam scaffold only:

```bash
# preview
bash "$(dirname "$0")/scripts/init.sh" --dry-run

# scaffold (never commits — you git add/commit when ready)
bash "$(dirname "$0")/scripts/init.sh"

# import closed/open issues from GitHub into foam/features/
bash "$(dirname "$0")/scripts/import-kanban.sh"
```

`init.sh` copies templates only. It does **not** run `git commit` — you decide
when to commit the scaffold.

## Agent protocol (every product/architecture/design task)

1. Read `AGENTS.md` and `foam/index.md` (Map of Content).
2. Search `foam/decisions/` for related ADRs (grep tags or keywords).
3. Search `prd/` and `plans/` for related specs.
4. Search **GitHub** for overlapping work before deciding or coding:
   ```bash
   gh issue list --state all --search "keywords" --limit 20
   gh pr list --state all --search "keywords" --limit 20
   ```
5. If an issue, PR, ADR, or PRD already covers the topic — read it and
   **confront** the new proposal with recorded decisions. Cite sources.
6. If the user requests something that **contradicts an accepted ADR**, cite the
   ADR and ask whether to supersede it (new ADR required).
7. After shipping or deciding: update foam, Kanban, and `AGENTS.md` when needed.

## Workflows

### A — New feature

1. Kanban: issue on board (`cda-kanban`).
2. PRD: `cda-prd/scripts/new-prd.sh <issue#>` → `prd/PRD-NNN-slug.md`.
3. Plan: `cda-plan/scripts/new-plan.sh <issue#>` → `plans/PLAN-NNN-slug.md`.
4. Board: **Ready** when PRD `approved`; claim → implement (`cda-kanban` B).
5. PR with `Closes #NNN`.
6. On merge: PRD → `shipped`; plan → `done`; `import-kanban.sh`; new ADR if needed.

### B — Architecture decision

1. Copy `foam/decisions/ADR-template.md` → `foam/decisions/ADR-NNN-slug.md`.
2. Fill context, decision, **alternatives rejected**, consequences.
3. Add row to `foam/decisions/index.md` and `foam/index.md`.
4. Link from relevant `foam/architecture/*.md` notes.

### C — Sync from Kanban

```bash
bash scripts/import-kanban.sh          # refresh shipped + backlog indexes
bash scripts/import-kanban.sh --dry-run
```

Regenerates `foam/features/shipped.md` and `foam/features/backlog.md` from
`gh issue list`. Re-run after milestones or when onboarding a repo.

### D — Import historical decisions (one-off)

For mature repos: read closed issues + CHANGELOG + `AGENTS.md` gotchas → distill
into ADRs and PRDs (do not dump every issue as an ADR).

## Integration with cda-kanban

| Kanban | Foam |
| ------ | ---- |
| Issue #N title/body | PRD source (`cda-prd`) |
| Status Ready | PRD `approved` + plan exists (`cda-plan`) |
| Status Done | PRD `shipped`, plan `done` |
| Epic / backlog | `foam/features/backlog.md` + PRDs |
| Claim + branch | Plan checklist drives tasks |

## Layout (repo root)

```
foam/
  index.md              # start here
  README.md
  decisions/            # ADRs
  product/                # vision, rules
  features/               # shipped / backlog catalog
  architecture/           # topic guides (optional)
prd/
plans/
.foam/templates/          # Foam editor templates
.cursor/rules/
  product-memory.mdc      # project rule (copied on init)
```

## Additional material

- [conventions.md](conventions.md) — naming, statuses, tags
- [reference.md](reference.md) — ADR/PRD/plan templates explained
- [templates/scaffold/](templates/scaffold/) — files copied by `init.sh`
