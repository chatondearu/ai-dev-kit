# PLAN-404 — docs(foam): record ADR-016 foam-first kanban and global board index

- **Status**: verify
- **PRD**: [[../prd/PRD-403-foam-first-kanban|PRD-403]]
- **GitHub**: [#404](https://github.com/chatondearu/ai-dev-kit/issues/404)
- **Date**: 2026-11-19

## Summary

Land ADR-016 (proposed until owner accept) and the `foam/board/` card
contract so skills (#405) and the cdamux index (#407) implement against one
schema. Link #265 as overlap only.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]] (this issue) | 1 card = 1 file; optional GH issues; no Projects SoT; meta auto-merge allowlist; `$CDAMUX_HOME/board-index.json` is a cache |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | Amend Ready listing + narrow "no auto-merge"; keep fast-lane / poller |
| [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]] | Foam browser stays read-only Fleet UI; do not reopen #265 |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Index lives under `$CDAMUX_HOME`; files readable with `cat`/`jq` |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- ADR-016, foam/board README + template, documentation-layout row, ADR-002
  amendment, foam indexes, dogfood cards for epic #403 children

### Out of scope

- Skill / orch / cdamux CLI code (#405–#407)
- Rollout to other repos (#408)
- `AGENTS.md` managed block (leave for #405 / `cda-agents` if routing text changes)
- User-facing `doc/` (not in auto-merge yet)

## Technical approach

Write the decision in `foam/decisions/` (status `proposed`). Put the card
contract next to the files that will be globbed. Point documentation-layout
at `foam/board/` as execution status. Add a short ADR-002 amendment so
agents who start at orch-auto still hit the new constraint.

## Tasks

- [x] ADR-016 records foam-first board, optional GH issues, no Projects SoT, worker meta auto-merge, global index cache; links #265 / #403
- [x] Card contract: YAML fields, status vocabulary, one-file-per-card, no hand-edited board index
- [x] documentation-layout lists `foam/board/` as execution status
- [x] ADR-002 amendment points at ADR-016
- [x] foam/index.md and decisions/index.md rows
- [x] Dogfood cards for #403–#408
- [x] Planner PR `Closes #404`; ADR stays `proposed` until owner accept on the PR

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Owner tweaks card fields on review | Status/priority/updated/github_issue are the constraint; extra fields stay optional |
| Dual SoT while #405 is open | ADR says Projects is not SoT now; #405 removes it from the default path |

## Test plan

Seams: foam graph links resolve; doc-hygiene still passes with `foam/board/` present.

- [ ] `bash skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh --check`
- [ ] Wikilinks from ADR-016 / PRD-403 / indexes exist on disk

## Verification

- [ ] Lint / test / build per project conventions (`bash scripts/verify.sh`)
- [ ] Update foam / ADR if a new decision was made (this plan *is* that ADR)
