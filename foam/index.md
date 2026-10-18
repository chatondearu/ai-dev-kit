# Project memory

Map of Content (MOC). Start here before product or architecture decisions.

## How to use

1. **New feature** → `cda-prd` → `prd/PRD-NNN-slug.md`, link ADRs.
2. **Implementation** → `cda-plan` → `plans/PLAN-NNN-slug.md`.
3. **Technical choice** → `foam/decisions/ADR-NNN-slug.md`.
4. **Sync kanban** → `import-kanban.sh` refreshes [[features/shipped|shipped]] / [[features/backlog|backlog]].

## Product

- [[product/vision|Vision & positioning]]

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

## PRDs & plans

| PRD | Status |
| --- | ------ |
| _(none yet)_ | |

- PRD template: `prd/template.md`
- Plan template: `plans/template.md`

## Tags

`#product` `#architecture` `#deployment` `#data` `#auth`
