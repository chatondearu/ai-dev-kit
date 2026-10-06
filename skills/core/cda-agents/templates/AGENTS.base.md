# Agent guide

Instructions for AI agents working in this repository.

<!-- agents-setup:managed -->

## Mandatory workflow

Every feature, fix, or decision that affects product, architecture, or design
must follow this sequence:

1. Read this `AGENTS.md`, then `foam/index.md` (if present).
2. Search **foam** (`foam/decisions/`, `prd/`, `plans/`) for related ADRs and specs.
3. Search **GitHub** for overlap before starting work:
   ```bash
   gh issue list --state all --search "keywords" --limit 20
   gh pr list --state all --search "keywords" --limit 20
   ```
4. If an issue, PR, ADR, or PRD already covers the topic — **read it** and
   confront the new proposal with recorded decisions. Cite sources.
5. For non-trivial scope: ensure a Kanban issue, PRD (`prd/`), and plan
   (`plans/`) exist or create them before coding.
6. After shipping or deciding: update foam (ADR, PRD status, feature lists) and
   refresh this file when stack or skills change.

Skills: **cda-foam**, **cda-prd**, **cda-plan**,
**cda-dev**, **cda-kanban**,
**cda-agents**, **git-commit**, **code-reviewer**.

## Project memory (foam)

| Path | Purpose |
| ---- | ------- |
| `foam/index.md` | Map of Content — start here |
| `foam/decisions/` | Architecture Decision Records |
| `prd/` | Product requirements |
| `plans/` | Implementation checklists |
| GitHub Project | Kanban execution status |

Sync feature lists: `cda-foam/scripts/import-kanban.sh`.

Cursor rule (if present): `.cursor/rules/product-memory.mdc`.

## Skills by context

| When | Skill |
| ---- | ----- |
| Bootstrap / update this file | `cda-agents` |
| ADRs, foam graph, import kanban | `cda-foam` |
| PRD from Kanban issue | `cda-prd` |
| Implementation plan from PRD | `cda-plan` |
| Multi-task chat batch → epic + parallel PRs | `cda-dev` |
| Issues, board, PRs, milestones | `cda-kanban` |
| Commits | `git-commit` |
| Code review | `code-reviewer` |
| Bold creative UI | `frontend-design` |
| Anti-slop landings / redesigns | `design-taste-frontend` |
| Brand DESIGN.md templates | `cda-design-md` |
| UI a11y / UX audit | `web-design-guidelines` |
| User-facing `doc/` | `technical-docs-sync` |
| Library docs | `context7` |
| Nix / direnv projects | `nix-develop-shell`, `nix-direnv-setup` |

<!-- TYPE_OVERLAY -->

<!-- /agents-setup:managed -->

## Project-specific notes

_Add stack versions, deployment URLs, gotchas, and links below. Agents may update
this section when they learn durable facts about the repo._
