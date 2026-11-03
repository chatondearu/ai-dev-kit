# Project memory

Map of Content (MOC). Start here before product or architecture decisions.

## How to use

1. **New feature** → `cda-prd` → `prd/PRD-NNN-slug.md`, link ADRs.
2. **Implementation** → `cda-plan` → `plans/PLAN-NNN-slug.md`.
3. **Technical choice** → `foam/decisions/ADR-NNN-slug.md`.
4. **Sync kanban** → `import-kanban.sh` refreshes [[features/shipped|shipped]] / [[features/backlog|backlog]].

**Where files live:** [[product/documentation-layout|Documentation layout]] (do not add
`docs/superpowers/` or PRDs/plans outside `prd/` / `plans/`).

## Product

- [[product/vision|Vision & positioning]]
- [[product/documentation-layout|Documentation layout (canonical paths)]]

## Features

- [[features/index|Feature catalog]]
- [[features/shipped|Shipped]]
- [[features/backlog|Backlog]]

## Architecture

- Operator docs (orch / headless / AFK): repo [`doc/`](../doc/README.md)
- _(Add topic notes under `foam/architecture/` and link ADRs here.)_

## Decisions (ADRs)

Index: [[decisions/index|All ADRs]]

- [[decisions/ADR-001-host-tools-vs-kit|ADR-001 — Host tools stay outside the kit]]
- [[decisions/ADR-002-orch-auto-and-fast-lane|ADR-002 — Orch auto poller and fast-lane]]
- [[decisions/ADR-003-cda-aitools|ADR-003 — cda.aitools companion (experimental)]]
- [[decisions/ADR-004-cdamux|ADR-004 — cdamux in-tree orchestrator]]
- [[decisions/ADR-005-cda-graph-orchestration|ADR-005 — CDA graph orchestration]]
- [[decisions/ADR-006-review-gate-fail-closed-verify|ADR-006 — Independent review gate and fail-closed verify]]
- [[graph/README|Foam machine graph + orch overlay]]

## PRDs & plans

| PRD | Status |
| --- | ------ |
| [[../prd/PRD-33-cda-aitools|PRD-33]] cda.aitools companion | shipped |
| [[../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]] kiss-audit orch+gh-board | shipped |
| [[../prd/PRD-6-kiss-audit-fixes|PRD-6]] prior kiss-audit batch | shipped |
| [[../prd/PRD-72-cdamux-worktree-session-chrome|PRD-72]] cdamux worktree session chrome | approved |
| [[../prd/PRD-75-cdamux-keymap-settings-form|PRD-75]] cdamux keymap + Settings form | shipped |
| [[../prd/PRD-79-cdamux-widgets-extract|PRD-79]] cdamux widgets extract | approved |
| [[../prd/PRD-83-cdamux-fleet-status-preview-agent|PRD-83]] cdamux Fleet status/preview/agent tag | approved |
| [[../prd/PRD-90-kiss-audit-2026-11|PRD-90]] kiss-audit 2026-11 fixes (plan: [[archive/plans/PLAN-90-kiss-audit-2026-11|PLAN-90]]) | shipped |
| [[../prd/PRD-129-kiss-audit-2026-11-03|PRD-129]] kiss-audit 2026-11-03 fixes (plan: [[../plans/PLAN-129-kiss-audit-2026-11-03|PLAN-129]]) | approved |
| [[../prd/PRD-155-cdamux-agent-map-ux|PRD-155]] cdamux agent map UX (plan: [[../plans/PLAN-155-cdamux-agent-map-ux|PLAN-155]]) | approved |

- PRD template: `prd/template.md`
- Plan template: `plans/template.md`

## Tags

`#product` `#architecture` `#deployment` `#data` `#auth`
