# Project memory (Foam)

Structured project memory for humans and AI agents. **Start at
[`index.md`](index.md).**

## Layout

```
foam/           # Knowledge graph (wikilinks)
  decisions/    # Architecture Decision Records (ADRs)
  product/      # Vision, rules
  features/     # Shipped / backlog (sync via import-kanban.sh)
  architecture/ # Topic guides linked to ADRs
prd/            # Product Requirements Documents
plans/          # Implementation plans
```

## Bootstrap

Installed via **ai-dev-kit** skill `foam-project-memory`:

```bash
bash ~/.cursor/skills/foam-project-memory/scripts/init.sh
bash ~/.cursor/skills/foam-project-memory/scripts/import-kanban.sh
```

## Workflows

See skill `foam-project-memory` and project `AGENTS.md` § Project memory.
