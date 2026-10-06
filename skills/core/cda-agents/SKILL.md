---
name: cda-agents
description: >-
  Bootstrap or update a repository AGENTS.md with mandatory foam + kanban
  workflow, project-type overlays (web, api, infra, fullstack), and skill
  references. Use when starting a project, onboarding agents, refreshing agent
  instructions after stack changes, or when the user asks to set up AGENTS.md.
---

# Project agents setup

Announce at start: **"Using cda-agents skill."**

## Purpose

Produce or refresh **`AGENTS.md`** at the repo root so every agent (Cursor,
Claude Code, opencode, Codex) shares the same:

- Mandatory **foam + kanban** workflow (read memory, search issues/PRs, confront decisions)
- **Skill** references from ai-dev-kit
- **Stack overlay** by project type (web, api, infra, fullstack, minimal)

Cursor **project rules** (`.cursor/rules/`) remain Cursor-only; `AGENTS.md` is the
portable contract.

## Bootstrap a repository

From the target repo root (or pass `TARGET_DIR`):

```bash
SKILL=~/.cursor/skills/cda-agents   # or ~/.claude/skills/...

# Full bootstrap: foam scaffold + AGENTS.md
bash "$SKILL/scripts/setup-agents.sh" --type auto --init-foam

# AGENTS.md only (foam already present)
bash "$SKILL/scripts/setup-agents.sh" --type web --force

# Preview
bash "$SKILL/scripts/setup-agents.sh" --dry-run --type auto
```

### Project types

| `--type` | Overlay |
| -------- | ------- |
| `auto` | Detect from `package.json`, `nuxt.config.*`, `pyproject.toml`, `flake.nix`, … |
| `web` | Vue/Nuxt, frontend-design |
| `api` | Backend contracts, technical-docs-sync |
| `infra` | Nix, direnv, deployment ADRs |
| `fullstack` | Combines web + api + infra hints |
| `minimal` | Base workflow only |

## Agent obligations (also written into AGENTS.md)

Before **any** feature or technical/product/design decision:

1. Read `AGENTS.md` and `foam/index.md`.
2. Search foam ADRs, PRDs, plans.
3. Search GitHub issues and PRs (`gh issue list`, `gh pr list`) for overlap.
4. Read existing material and **confront** the new proposal.
5. Update foam + Kanban artifacts after shipping or deciding.
6. **Update `AGENTS.md`** when stack, skills, or durable gotchas change.

## Integration

| Skill | Role |
| ----- | ---- |
| `cda-foam` | `foam/`, ADRs; `init.sh` with `--init-foam` |
| `cda-prd` | PRD from Kanban issue |
| `cda-plan` | Plan from approved PRD |
| `cda-kanban` | Issues, board, PRs |
| `technical-docs-sync` | User-facing `doc/` (listed in overlays) |

## Templates

- [templates/AGENTS.base.md](templates/AGENTS.base.md) — managed block (markers)
- [templates/overlays/](templates/overlays/) — per-type sections

Re-run `setup-agents.sh` after editing templates in ai-dev-kit to propagate to
projects (`--force`).

## Updating AGENTS.md during work

When you learn durable facts (versions, URLs, env quirks, required skills),
append or edit **§ Project-specific notes** — outside the managed block so
`setup-agents.sh --force` does not erase them.

When workflow or skill lists change in ai-dev-kit, re-run:

```bash
bash scripts/setup-agents.sh --type auto --force
```
