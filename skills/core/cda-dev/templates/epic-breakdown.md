# Epic breakdown (validate before GO)

> Fill during Phase 0. Present to the user and wait for **GO**.
> English content. Do not create issues/PRD until GO.

## Epic

| Field | Value |
| ----- | ----- |
| Working title | |
| Objective | |
| Target repo / area | |
| Non-goals | |
| Success criteria | |
| Related ADRs (wikilinks or paths) | |
| Open issue/PR overlaps | |

## Children

| # | Type | Scope | Title (imperative) | Size | Priority | Depends on | Notes |
| - | ---- | ----- | ------------------ | ---- | -------- | ---------- | ----- |
| 1 | feat/fix/chore/docs | | | XS–XL | P0–P2 | — / row # | |
| 2 | | | | | | | |

> **Depends on** is by row number here. After GO, when rows become issues, each
> dependency is written on the dependent child's body as `Blocked by: #<issue>`
> (see cda-dev conventions, "Blocking edges and the frontier"). `orch.sh` reads
> those edges to gate dispatch — a blocked child is held until its blocker closes.
>
> Prefer **tracer-bullet children**: each a thin vertical slice that ships
> something observable end to end, not a horizontal layer (all types, then all UI).
> Fewer, deeper edges beat a wide fan of shallow ones. See the `tdd` skill.

## Cross-cutting constraints

- …

## Risks / BLOCKED candidates

- …

## GO

- [ ] User approved this breakdown (**GO**)
- [ ] Date / session note:
