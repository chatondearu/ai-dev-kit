# Foam project memory — conventions

## Paths

All paths are **repo root** relative:

| Path | Purpose |
| ---- | ------- |
| `foam/` | Knowledge graph (not `docs/foam/`) |
| `prd/` | Product requirements |
| `plans/` | Implementation plans |
| `.foam/templates/` | Foam VS Code snippet templates |

## ADR naming

- File: `foam/decisions/ADR-NNN-short-slug.md` (NNN zero-padded to 3 digits)
- Title: `# ADR-NNN — Short title`
- Status: `proposed` | `accepted` | `deprecated` | `superseded`
- Always include: Context, Decision, Alternatives considered, Consequences
- Supersede by linking the replacement ADR; never silently delete old ADRs

## PRD naming

- File: `prd/PRD-NNN-short-slug.md` (NNN = GitHub issue number when applicable)
- Status: `draft` | `review` | `approved` | `shipped` | `partial` | `backlog`

## Plan naming

- File: `plans/PLAN-NNN-short-slug.md`
- Status: `draft` | `in-progress` | `verify` | `done`
- Must link to PRD and list ADR constraints in a table

## Wikilinks

Foam wikilinks connect notes:

```markdown
[[decisions/ADR-001-example|ADR-001]]
[[../prd/PRD-042-feature|PRD-042]]
```

Tags at top of notes: `#product` `#architecture` `#deployment` etc.

## Language

- **All** foam, PRD, plan, and ADR content: **English**
- Code comments: English (project may have other rules)
- Chat with user: follow user language

## What deserves an ADR

Record when:

- Choosing between multiple valid architectures
- Constraint will bind future work (transport, auth, storage, deploy)
- Rejected alternative might be proposed again

Do **not** create an ADR for every closed issue — use `foam/features/shipped.md`.

## AGENTS.md

After `init.sh`, merge `AGENTS.md.snippet` into the project `AGENTS.md` or
equivalent agent guide. Point agents to `foam/index.md`.
