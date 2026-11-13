# PLAN-266 — Record ADR: Foam browser replaces Notes sidecar

- **Status**: done
- **PRD**: [[../prd/PRD-265-cdamux-foam-browser|PRD-265]]
- **GitHub**: [#266](https://github.com/chatondearu/ai-dev-kit/issues/266)
- **Date**: 2026-11-13

## Summary

Accept an ADR that the Fleet Notes sidecar is replaced by a read-only Foam
browser under `<project>/foam/`, and narrowly supersede the Teep/file-rail
non-goal (PRD-39/41) for that scope only.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | cdamux stays local / in-tree product; no cloud notes store |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Fleet “graph” = orch run DAG, not Foam wikilinks |
| [[../foam/product/documentation-layout|documentation-layout]] | foam/prd/plans/doc paths stay canonical |

## Scope

### In scope

- New accepted ADR (next free number, expected ADR-012)
- Update `foam/index.md` (and `foam/decisions/index.md` if present)
- Comment on #266 / #265 with ADR path

### Out of scope

- CDAMUX code changes (owned by #267–#269)
- Deleting CLI notes

## Technical approach

Follow `cda-foam` ADR template; confront PRD-39/41 Teep non-goal and state the
narrow supersede explicitly. English only.

## Tasks

- [x] Draft ADR with context (Notes = scratch; foam = project memory), decision
      (replace sidecar; read-only; git-toplevel + `foam/`; Teep narrow supersede),
      alternatives (dual pane, rename-only, full Teep), consequences
- [x] Link GitHub #265 / #266 and PRD-265
- [x] Update foam MOC / decisions index
- [x] Comment on the issue with the ADR path

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| ADR too broad (generic file rail) | Explicit “foam/ only; Teep remains out of scope” |

## Test plan

- [x] ADR readable from `foam/index.md` wikilink
- [x] No contradiction left unstated vs ADR-005 graph naming

## Verification

- [x] Docs-only change; `nix develop -c bash scripts/verify.sh` if required by hooks
- [ ] PR `Closes #266`
