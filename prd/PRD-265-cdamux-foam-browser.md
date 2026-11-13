# PRD-265 — CDAMUX Foam browser replaces Notes sidecar

- **Status**: approved
- **GitHub**: [#265](https://github.com/chatondearu/ai-dev-kit/issues/265)
- **Date**: 2026-11-13
- **Tags**: #product #cdamux #foam

## Problem

Fleet’s sidecar **Notes** tab shows a single operator scratch file
(`.cdamux/notes.md` or central slug), plain text, truncated — not the project’s
versioned memory. In this kit, **`foam/`** is the source of truth for ADRs, MOC,
and agent memory; session and agent links often point at those markdown files.
Operators want to browse and preview foam inside CDAMUX instead of a parallel
notes store in the same UI slot.

## Goals

- [ ] Replace the Fleet sidecar Notes surface with a **read-only Foam browser**
      rooted at `<project>/foam/`.
- [ ] Anchor on **git toplevel / discovered project**, not raw pane cwd.
- [ ] Preview selected foam markdown in the sidecar (reuse pulldown-cmark path).
- [ ] Deep-link: a `foam/...` path from session context opens that file in the
      Foam tab (in-app only).
- [ ] Record an ADR that replaces Notes for this slot and **narrowly supersedes**
      the Teep/file-rail non-goal in PRD-39/41 for `foam/` only.

## Non-goals

- In-app editing of foam (v0 = read-only; external `$EDITOR` may come later).
- Browsing `prd/`, `plans/`, `doc/`, or generated HTML outside `foam/`.
- Full Teep-style generic file rail.
- Foam wikigraph visualization (distinct from Fleet orch graph).
- Resolving `[[wikilinks]]` in v0.
- Removing or migrating CLI `cdamux notes` / `.cdamux/notes.md` (legacy until a
  follow-up).
- Changing orch graph behaviour (ADR-005).

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer in Fleet | Selects a session on a kit repo → Foam tab lists `foam/**/*.md` → opens `foam/index.md` with rendered preview |
| Maintainer | Missing `foam/` → clear empty state, no crash |
| Maintainer | Preview/context exposes `foam/decisions/ADR-004-….md` → Foam tab focuses that file |
| Operator with legacy notes | Still uses `cdamux notes edit` for scratch; sidecar no longer shows that file |

## Requirements

### Functional

- [ ] Sidecar tab formerly labelled Notes becomes **Foam** (or equivalent) and
      lists markdown under the anchored project’s `foam/` tree.
- [ ] Selecting a listed file shows a markdown preview in the sidecar.
- [ ] Empty or missing `foam/` yields an explicit empty/error state.
- [ ] Deep-link from a resolvable `foam/...` path focuses that file; non-foam
      paths do not hijack the tab.
- [ ] No CDAMUX code path writes into `foam/` from this feature.

### Non-functional

- [ ] Local filesystem only (ADR-004); no cloud sync / notes DB.
- [ ] Verify: `nix develop -c bash scripts/verify.sh` green on shipping PRs.
- [ ] TUI snapshots / tests updated where the sidecar chrome changes.

## Children

| Issue | Scope |
| ----- | ----- |
| [#266](https://github.com/chatondearu/ai-dev-kit/issues/266) | ADR + foam index |
| [#267](https://github.com/chatondearu/ai-dev-kit/issues/267) | Resolve `foam/` + replace Notes tab with browser |
| [#268](https://github.com/chatondearu/ai-dev-kit/issues/268) | Render selected foam markdown |
| [#269](https://github.com/chatondearu/ai-dev-kit/issues/269) | Deep-link foam paths into Foam tab |

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]],
  [[../foam/decisions/ADR-008-graphify-code-graph|ADR-008]] (`CDA_FOAM_HOME` for
  skills — not required for CDAMUX v0 root), upcoming ADR from #266
- Product: [[../foam/product/documentation-layout|Documentation layout]]
- Prior Notes: #47, #62; PRD-39 / PRD-41 Teep non-goal (narrow supersede)
- Architecture: `cdamux/docs/architecture.md` `[notes]` section (update when
  shipping)

## Open questions

1. _(Assumed)_ No `[[wikilink]]` resolution in v0 — plain path navigation only.
2. _(Assumed)_ CLI `notes` remains until a dedicated follow-up.

## Success metrics

- Dogfood: open this repo in Fleet → Foam tab shows this foam tree and readable
  ADR preview without leaving CDAMUX.
- No regression of orch graph tab; no accidental writes under `foam/`.

## Implementation pointer

Child plans: `plans/PLAN-266-*.md`, `plans/PLAN-267-*.md`, `plans/PLAN-268-*.md`,
`plans/PLAN-269-*.md`.
