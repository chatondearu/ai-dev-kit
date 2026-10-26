# PLAN-48 — cdamux dogfood sign-off & closure

- **Status**: done
- **PRD**: [[../prd/PRD-39-cdamux|PRD-39]]
- **GitHub**: [#48](https://github.com/chatondearu/ai-dev-kit/issues/48)
- **Date**: 2026-10-26

## Summary

Human dogfood on maintainer host; mark PRD/plans done; validate extract checklist
(ADR-004) without enabling `aitools` Nix.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | No default aitools package |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Extract only after dogfood metrics |

## Scope

### In scope

- Checklist evidence; foam/PRD status; epic #40 summary

### Out of scope

- `install.sh`; creating GitHub repo (human step OK)

## Tasks

- [x] Complete `cdamux/docs/dogfood.md` sign-off section; link evidence on #48 / #40
- [x] PRD-39 goals checked; PLAN-39 + PLAN-41–44 → done; #45–#47 remain open
- [x] `prompts/next-agent.md` → maintenance / extract mode
- [x] Review `cdamux/docs/extract-repo.md` steps; file issue for repo split
- [x] Epic #40 comment: shipped summary (via PR + issue comment)

## Test plan

- N/A (process); re-run full `cargo test` on dogfood host

## Verification

- [x] Maintainer sign-off comment requested in `dogfood.md`; no open P0 on milestone `cdamux-v1`
