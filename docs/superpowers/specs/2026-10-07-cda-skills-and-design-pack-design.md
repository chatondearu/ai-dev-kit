# Design: `cda-*` core skills + hybrid design pack

Date: 2026-10-07  
Status: approved

## Goals

1. Make chatondearu-owned workflow skills easy to spot and invoke via a short `cda-` prefix.
2. Enrich the kit with curated design skills without vendoring entire competing harnesses.

## Decisions

| Decision | Choice |
| -------- | ------ |
| Rename scope | Core workflow only |
| Old names | Hard cut (no deprecated aliases) |
| External projects | Hybrid: vendor useful design assets; catalogue ECC / Superdesign |
| Rename mechanics | Physical `git mv` so install basename = skill name |

## Core rename map

| Old | New |
| --- | --- |
| `dev-orchestrator` | `cda-dev` |
| `github-kanban-orchestrator` | `cda-kanban` |
| `foam-project-memory` | `cda-foam` |
| `foam-prd` | `cda-prd` |
| `foam-plan` | `cda-plan` |
| `project-agents-setup` | `cda-agents` |
| `foam-batch-orchestrator` | Removed (was already an alias of `cda-dev`) |

Unchanged: `nix-*`, `git-commit`, `code-reviewer`, `technical-docs-sync`, `frontend-design`, `context7`.

Convention: `cda-*` marks kit-owned workflow (and kit-authored design helpers such as `cda-design-md`). Third-party vendored skills keep upstream `name` values for easier sync.

## Design pack (vendored)

| Skill | Source | Role |
| ----- | ------ | ---- |
| `design-taste-frontend` | Leonxlnx/taste-skill | Anti-slop direction + brief inference |
| `web-design-guidelines` | vercel-labs/agent-skills | Interaction / a11y audit checklist |
| `cda-design-md` | VoltAgent/awesome-design-md (subset) | Drop-in brand `DESIGN.md` templates |

`frontend-design` stays for bold creative UI work. Taste steers direction; Vercel guidelines audit quality.

## Catalogue only (`docs/recommended.md`)

- **ECC** (`affaan-m/ECC`): full agent harness — install separately, do not vendor into this kit.
- **Superdesign image-to-code**: SaaS product; optional local alternative exists upstream in taste-skill (`image-to-code-skill`).
- Upstream awesome-design-md for extending the curated brand set.

## Install / migration

After rename: `./install.sh --force` so old symlinks are replaced by `cda-*` targets. No special mapping in `install.sh` or `nix/maid.nix`.

## Out of scope

- Renaming quality/dev/tools skills to `cda-*`
- Vendoring the full ECC tree or entire awesome-design-md catalogue
- Auto-merge or CI changes unrelated to skill naming
