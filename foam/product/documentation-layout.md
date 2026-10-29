# Documentation layout (canonical)

#product #architecture

English only for all paths below. Do **not** create parallel trees (e.g.
`docs/superpowers/`, random `specs/` folders, PRDs under `skills/`).

| Layer | Path | Role |
| ----- | ---- | ---- |
| Agent memory (MOC, ADRs, catalogs) | `foam/` | Start at [[../index|index.md]] |
| Product requirements | `prd/PRD-NNN-slug.md` | Via `cda-prd` / Kanban issue # |
| Implementation checklists | `plans/PLAN-NNN-slug.md` | Via `cda-plan`; link PRD + ADRs |
| Operator docs (humans) | `doc/` | Via `cda-docs`; mirrors runtime |
| External tools (not vendored) | `docs/recommended.md` | Host CLIs, optional services |
| Subproject operator docs | e.g. `cdamux/docs/` | Scoped to that crate/app |
| Skill internals | `skills/**/SKILL.md` | Behavior contract; link `doc/` / foam |

## Agent protocol

1. Read `AGENTS.md` and [[../index|foam/index.md]].
2. Search `foam/decisions/`, then `prd/`, then `plans/`.
3. User-facing changes → update `doc/` (not foam prose duplicates).

## Historical material

Pre–cda-foam brainstorming from Cursor Superpowers lives under
[[../archive/README|foam/archive/]] (read-only context; canonical specs are
PRD/plan/ADR/`doc/`).

## Forbidden patterns

- New PRDs or plans outside `prd/` and `plans/`.
- New ADRs outside `foam/decisions/`.
- Duplicating operator docs in foam (wikilink instead).
- `docs/superpowers/**` (removed; use foam archive + kit workflows).

Skills: **cda-foam**, **cda-prd**, **cda-plan**, **cda-docs**, **cda-agents**.
