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
- [[decisions/ADR-007-cdamux-session-hierarchy|ADR-007 — cdamux session hierarchy]]
- [[decisions/ADR-008-graphify-code-graph|ADR-008 — Graphify code graph via cda.aitools]]
- [[decisions/ADR-009-cdamux-session-restore-registry|ADR-009 — cdamux session restore registry]]
- [[decisions/ADR-010-worker-cli-inheritance-and-tiers|ADR-010 — Worker CLI inheritance and tiers]]
- [[decisions/ADR-011-cdamux-unified-state-model|ADR-011 — cdamux unified state model]]
- [[decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012 — Foam browser replaces Notes sidecar]]
- [[decisions/ADR-013-mise-alongside-nix-devshell|ADR-013 — Mise alongside Nix for daily Node/Rust]]
- [[graph/README|Foam machine graph + orch overlay]]

## PRDs & plans

| PRD | Status |
| --- | ------ |
| [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]] | approved |
| [[../prd/PRD-307-finalize-stale-session-kill|PRD-307-finalize-stale-session-kill]] | approved |
| [[../prd/PRD-293-cdamux-fast-popup-open|PRD-293]] fast Alt-P / dashboard open (plans: [[../plans/PLAN-294-perf-cdamux-lazy-load-foam-sidecar-until-foam-tab-selected|PLAN-294]], [[../plans/PLAN-295-perf-cdamux-bound-run-agent-listing-on-dashboard-open|PLAN-295]], [[../plans/PLAN-296-chore-cdamux-first-paint-timing-hook-dogfood-alt-p-checklist|PLAN-296]], [[../plans/PLAN-297-feat-cdamux-optional-fast-alt-p-bind-via-cdamux-pick|PLAN-297]]) | approved |
| [[../prd/PRD-290-mise-alongside-nix|PRD-290]] Mise alongside Nix (plan: [[../plans/PLAN-290-feat-kit-add-mise-toml-and-optional-mise-run-verify-adr-013|PLAN-290]]) | approved |
| [[../prd/PRD-239-orchestrator-session|PRD-239]] orchestrator session (unified state; slice 4 last: [[../plans/PLAN-257-cdamux-fleet-orch-ui|PLAN-257]], [[../plans/PLAN-258-cdamux-orch-restore|PLAN-258]], [[../plans/PLAN-259-cdamux-state-gc|PLAN-259]]) | shipped |
| [[../prd/PRD-33-cda-aitools|PRD-33]] cda.aitools companion | shipped |
| [[../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]] kiss-audit orch+gh-board | shipped |
| [[../prd/PRD-6-kiss-audit-fixes|PRD-6]] prior kiss-audit batch | shipped |
| [[../prd/PRD-72-cdamux-worktree-session-chrome|PRD-72]] cdamux worktree session chrome | approved |
| [[../prd/PRD-75-cdamux-keymap-settings-form|PRD-75]] cdamux keymap + Settings form | shipped |
| [[../prd/PRD-79-cdamux-widgets-extract|PRD-79]] cdamux widgets extract | approved |
| [[../prd/PRD-83-cdamux-fleet-status-preview-agent|PRD-83]] cdamux Fleet status/preview/agent tag | approved |
| [[../prd/PRD-90-kiss-audit-2026-11|PRD-90]] kiss-audit 2026-11 fixes (plan: [[archive/plans/PLAN-90-kiss-audit-2026-11|PLAN-90]]) | shipped |
| [[../prd/PRD-129-kiss-audit-2026-11-03|PRD-129]] kiss-audit 2026-11-03 fixes (plan: [[archive/plans/PLAN-129-kiss-audit-2026-11-03|PLAN-129]]) | shipped |
| [[../prd/PRD-155-cdamux-agent-map-ux|PRD-155]] cdamux agent map UX (plan: [[archive/plans/PLAN-155-cdamux-agent-map-ux|PLAN-155]]) | shipped |
| [[../prd/PRD-195-cdamux-session-resume|PRD-195]] cdamux session resume (plan: [[../plans/PLAN-195-cdamux-session-resume|PLAN-195]]) | approved |
| [[../prd/PRD-265-cdamux-foam-browser|PRD-265]] cdamux Foam browser replaces Notes (plans: [[../plans/PLAN-266-docs-foam-record-adr-foam-browser-replaces-notes-sidecar|PLAN-266]], [[../plans/PLAN-267-feat-cdamux-resolve-foam-and-replace-notes-tab-with-foam-bro|PLAN-267]], [[../plans/PLAN-268-feat-cdamux-render-selected-foam-markdown-in-sidecar|PLAN-268]], [[../plans/PLAN-269-feat-cdamux-deep-link-foam-paths-into-foam-tab|PLAN-269]]) | approved |

- PRD template: `prd/template.md`
- Plan template: `plans/template.md`

## Tags

`#product` `#architecture` `#deployment` `#data` `#auth`
