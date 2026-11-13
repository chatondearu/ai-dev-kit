# ADR-012 — Foam browser replaces Fleet Notes sidecar

- **Status**: accepted
- **Date**: 2026-11-13
- **Tags**: #architecture #cdamux #foam
- **GitHub**: [#266](https://github.com/chatondearu/ai-dev-kit/issues/266)
  (epic [#265](https://github.com/chatondearu/ai-dev-kit/issues/265))

## Context

Fleet’s sidecar **Notes** tab shows a single operator scratch file
(`.cdamux/notes.md` or a central slug): plain text, truncated, not the project’s
versioned memory. In this kit, **`foam/`** is the source of truth for ADRs, MOC,
and agent decision memory ([documentation layout](../product/documentation-layout.md)).

PRD-39 / PRD-41 listed a **Teep-style file rail** (generic filesystem browser /
sidecar process) as a non-goal. That non-goal must not block a **narrow**
read-only browser of the project’s `foam/` tree in the same UI slot Notes
occupied.

Naming: Fleet’s orch **graph** tab is the run DAG ([[ADR-005-cda-graph-orchestration|ADR-005]]);
Foam wikilinks are project memory, not that graph. This ADR does not redefine
orch graph behaviour.

## Decision

1. **Replace the Fleet Notes sidecar surface** with a **read-only Foam browser**
   rooted at `<project>/foam/` (markdown under that tree only).
2. **Project anchor** is git toplevel / discovered project root — not the raw
   pane cwd — so worktree sessions still resolve the kit’s foam tree.
3. **Read-only in-app**: no CDAMUX write path into `foam/` from this feature.
   Preview reuses the existing markdown render path (pulldown-cmark) where
   applicable. `[[wikilink]]` following was out of scope for v0 and is now in
   scope (see Amendment).
4. **Narrow supersede of the Teep / file-rail non-goal** (PRD-39 / PRD-41):
   browsing **`foam/` only** in this sidecar slot is in scope. A generic Teep
   file rail, browsing `prd/` / `plans/` / `doc/`, or a sidecar process remain
   **out of scope**.
5. **CLI Notes stay**: `cdamux notes` / `.cdamux/notes.md` remain for operator
   scratch until a dedicated follow-up; the sidecar no longer shows that file.
6. **Local filesystem only** ([[ADR-004-cdamux|ADR-004]]): no cloud notes store.

Implementation children: #267 (resolve + tab), #268 (render), #269 (deep-link).

## Amendment — 2026-11-14 (tree navigation and wikilinks, #317)

[PRD-315](../../prd/PRD-315-cdamux-orchestration-cockpit.md) brings **following
`[[wikilinks]]`** into scope; the flat list becomes a collapsible tree.

- The Foam tab shows `foam/` as a **collapsible tree** (directories + `.md`),
  navigable with keys (`t` focuses it; arrows / `j k h l`, `Enter`) and mouse
  clicks on rows. Existing caps (500 files, 64 KiB preview, 120 lines) stay.
- `[[name]]`, `[[path|label]]` and `[[name#heading]]` found in the opened file
  are listed under the preview. Targets resolve **only against the capped
  `foam/` listing**: exact `foam/<target>.md` path first, then the first file
  whose stem matches (case-insensitive). Anything else shows as unresolved and
  is never read — links cannot escape `foam/`.
- `Enter` on a resolved link opens it; `b` / `Backspace` returns through an
  in-memory history (50 entries). No write path is added.
- Still out of scope: editing, graph view, other roots, raw/rendered toggle
  (#318).

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Dual pane (Notes + Foam) | Two stores in one slot; Notes already fails as project memory |
| Rename-only (“Notes” → “Foam” still bound to `.cdamux/notes.md`) | Does not surface versioned foam; false branding |
| Full Teep-style generic file rail | Explicit non-goal in PRD-39/41; scope creep beyond project memory |

## Consequences

- **Positive**: Operators browse ADRs / MOC inside Fleet without leaving the TUI;
  foam remains the single project-memory root.
- **Negative**: Scratch notes leave the sidecar until a follow-up migrates or
  retires CLI notes; empty/`foam/` missing needs a clear empty state.
- **Constraint**: Do not expand the tab into a generic file browser; do not
  conflate Foam navigation with the orch graph (ADR-005); do not write foam
  from CDAMUX in this feature.

## References

- PRD: [[../../prd/PRD-265-cdamux-foam-browser|PRD-265]]
- Plan: [[../../plans/PLAN-266-docs-foam-record-adr-foam-browser-replaces-notes-sidecar|PLAN-266]]
- Prior non-goals: [[../../prd/PRD-39-cdamux|PRD-39]],
  [[../../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- Related: [[ADR-004-cdamux|ADR-004]], [[ADR-005-cda-graph-orchestration|ADR-005]],
  [[../product/documentation-layout|documentation-layout]]
