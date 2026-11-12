# Agent guide

Instructions for AI agents working in this repository.

<!-- agents-setup:managed -->

## Mandatory workflow

Every feature, fix, or decision that affects product, architecture, or design
must follow this sequence:

1. Follow the foam protocol (`cda-foam` § Protocol): read this file and
   `foam/index.md`, search ADRs/PRDs/plans and GitHub issues/PRs, confront the
   proposal with recorded decisions, cite sources.
2. Any feature or fix requested in chat — one subject or several — goes through
   `cda-dev` intake (`cda-dev/prompts/intake.md`): one round of questions, a
   breakdown, GO. A single child issue is fine. Do not start coding from chat,
   except a trivial fix (typo, one-liner, obvious small fix): branch → commit → PR.
3. For non-trivial scope, before coding a Kanban issue, `prd/PRD-<n>-*.md` and
   `plans/PLAN-<n>-*.md` exist (create the missing ones). A fast-lane Ready
   issue may use its triage agent brief instead (`cda-dev`, `cda-triage`).
4. After shipping or deciding: update foam (ADR, PRD status, feature lists) and
   refresh this file when stack or skills change.

Skills: **cda-foam**, **cda-prd**, **cda-plan**,
**cda-dev**, **cda-graph**, **cda-kanban**, **cda-doc-hygiene**,
**cda-agents**, **cda-commit**, **cda-review**.

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
| Feature / fix request in chat (one or many) → intake, PRs | `cda-dev` |
| Interactive cdamux orchestrator session | `cda-orchestrator` |
| Graph orchestration (parallel workers, graph-plan) | `cda-graph` |
| Issues, board, PRs, milestones | `cda-kanban` |
| Foam / PRD / plan layout verify | `cda-doc-hygiene` |
| Commits | `cda-commit` |
| Code review | `cda-review` |
| Bold creative UI | `cda-design-bold` |
| Anti-slop landings / redesigns | `cda-design-taste` |
| Brand DESIGN.md templates | `cda-design-md` |
| UI a11y / UX audit | `cda-design-audit` |
| Visual design pass (screenshot loop) | `cda-design-loop` |
| User-facing `doc/` | `cda-docs` |
| Library docs | `cda-context7` |
| Nix / direnv projects | `nix-develop-shell`, `nix-direnv-setup` |

<!-- TYPE_OVERLAY -->

<!-- /agents-setup:managed -->

## Project-specific notes

_Add stack versions, deployment URLs, gotchas, and links below. Agents may update
this section when they learn durable facts about the repo._
