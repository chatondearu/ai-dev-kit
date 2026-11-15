# PLAN-362 — docs(cda-dev,cda-graph): merge the two no-harness fallbacks

- **Status**: done
- **PRD**: [[../prd/PRD-356-orchestrator-actually-orchestrates|PRD-356]]
- **GitHub**: [#362](https://github.com/chatondearu/ai-dev-kit/issues/362)
- **Date**: 2026-11-15

## Summary

`cda-graph` § Cursor session fallback is the single no-harness procedure; `cda-dev`
§ Skill-only fallback keeps its hard rules and links to it.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Graph files only via `graph-node.sh` |

## Scope

### In scope

- `skills/core/cda-dev/SKILL.md` § Skill-only fallback
- `skills/core/cda-graph/SKILL.md` § Cursor session fallback (add any step only `cda-dev` had)

### Out of scope

- Harness code; `doc/` beyond fixing a broken link

## Tasks

- [x] Move missing steps (worktrees, verify, CI watch) into `cda-graph` section
- [x] Shrink `cda-dev` section to rules + link
- [x] `bash scripts/verify.sh`; PR `Closes #362`

## Test plan

- [x] `bash scripts/verify.sh` (link checks)
