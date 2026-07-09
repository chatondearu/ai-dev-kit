# Foam project memory — reference

## ADR template (minimum)

```markdown
# ADR-NNN — Title

- **Status**: proposed
- **Date**: YYYY-MM-DD
- **Tags**: #tag

## Context
Why now?

## Decision
What we will do.

## Alternatives considered
| Alternative | Why rejected |
| ----------- | ------------ |
| … | … |

## Consequences
- **Positive**: …
- **Negative**: …
- **Constraint**: …

## References
- Issues, PRs, code paths
```

## PRD template (minimum)

Problem, goals, non-goals, requirements, related ADRs, open questions, success
metrics, pointer to plan.

## Plan template (minimum)

Summary, ADR constraint table, scope in/out, technical approach by package/layer,
task checklist, risks, test plan, verification before merge.

## MOC (`foam/index.md`)

Sections to maintain:

1. How to use (link PRD/plan templates)
2. Feature catalog → `features/`
3. Product notes
4. Architecture notes
5. ADR index table
6. PRD index table
7. Tag legend

## Optional Foam editor

Install [Foam](https://marketplace.visualstudio.com/items?itemName=foam.foam-vscode)
for wikilink autocomplete and graph view. `.vscode/settings.json` in the scaffold
enables Foam when present. Plain Markdown works without the extension.

## Difference from technical-docs-sync

| Skill | Output | Audience |
| ----- | ------ | -------- |
| `technical-docs-sync` | `doc/` user docs | Humans operators |
| `foam-project-memory` | `foam/` + ADR/PRD | Agents + team decisions |

Link between them when useful; do not merge into one tree.

## import-kanban.sh output

Overwrites (regenerates):

- `foam/features/shipped.md` — `state:CLOSED` issues
- `foam/features/backlog.md` — open issues grouped by state

Does not create ADRs automatically. Agent or human distills epics into ADRs/PRDs.
