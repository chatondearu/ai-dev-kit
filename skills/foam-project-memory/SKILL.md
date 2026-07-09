---
name: foam-project-memory
description: >-
  Bootstrap and maintain a Foam-style project memory graph (ADR, PRD, plans) at
  repo-root foam/, wired to GitHub Kanban. Use when starting project memory,
  recording architecture decisions, drafting PRDs/plans, importing kanban issues
  into foam, or before product/architecture changes so the agent can challenge
  against accepted ADRs.
---

# Foam project memory

Announce at start: **"Using foam-project-memory skill."**

## What this is

A **versioned knowledge graph** in Markdown (wikilinks `[[note]]`) so agents and
humans share the same memory:

| Layer | Path | Role |
| ----- | ---- | ---- |
| **Foam graph** | `foam/` | MOC, ADRs, product notes, feature catalog |
| **PRD** | `prd/` | What / why per feature |
| **Plan** | `plans/` | How / when (checklist derived from PRD) |
| **Kanban** | GitHub Projects | Execution status (use `github-kanban-orchestrator`) |

`foam/` is **agent decision memory**. User-facing docs may live elsewhere (`doc/`,
`README.md`) — do not duplicate; link from `foam/` when needed.

## Bootstrap a repository

From the target repo root:

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

## Agent protocol (every product/architecture task)

1. Read `foam/index.md` (Map of Content).
2. Search `foam/decisions/` for related ADRs (grep tags or keywords).
3. If a PRD exists under `prd/` for this work, follow it.
4. If a plan exists under `plans/`, use it as the implementation checklist.
5. If the user requests something that **contradicts an accepted ADR**, cite the
   ADR and ask whether to supersede it (new ADR required).

## Workflows

### A — New feature

1. Kanban: issue in **Ready** (`github-kanban-orchestrator`).
2. Copy `prd/template.md` → `prd/PRD-NNN-slug.md`; link ADRs + foam notes.
3. Copy `plans/template.md` → `plans/PLAN-NNN-slug.md`.
4. Implement; PR with `Closes #NNN`.
5. On merge: PRD → `shipped`; add row to `foam/features/shipped.md`; new
   technical choice → new ADR.

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

## Integration with github-kanban-orchestrator

| Kanban | Foam |
| ------ | ---- |
| Issue #N title/body | PRD source material |
| Status Done | Row in `foam/features/shipped.md` |
| Epic / backlog | `foam/features/backlog.md` + `prd/PRD-NNN-*.md` |
| Claim + branch | `plans/PLAN-NNN-*.md` checklist |

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
