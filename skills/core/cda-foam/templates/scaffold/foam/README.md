# Project memory (Foam)

Structured project memory for humans and AI agents. **Start at
[`index.md`](index.md).**

## Layout

```
foam/           # Knowledge graph (wikilinks)
  board/        # Execution status (one card per file)
  decisions/    # Architecture Decision Records (ADRs)
  product/      # Vision, rules
  features/     # Shipped / backlog (sync via import-kanban.sh)
  architecture/ # Topic guides linked to ADRs
prd/            # Product Requirements Documents
plans/          # Implementation plans
```

## Bootstrap

Installed via **ai-dev-kit** skill `cda-foam`:

```bash
bash ~/.cursor/skills/cda-foam/scripts/init.sh
bash ~/.cursor/skills/cda-foam/scripts/import-kanban.sh
```

## Workflows

See skill `cda-foam` and project `AGENTS.md` § Project memory.
