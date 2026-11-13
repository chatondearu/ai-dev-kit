# PLAN-317 — feat(cdamux): Foam tab tree with keyboard and mouse navigation, follow wikilinks

- **Status**: ready
- **PRD**: [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]]
- **GitHub**: [#317](https://github.com/chatondearu/ai-dev-kit/issues/317)
- **Date**: 2026-11-14

## Summary

Replace the flat Foam list with a collapsible tree of `foam/`, navigable with keys and mouse; follow `[[wikilinks]]` from the preview.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]] | Read-only, `foam/` only; amend for wikilink following |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local FS only |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Tree model (dirs + `.md`), expand/collapse, selection, existing caps
- Keys: up/down, left/right collapse/expand, Enter open; mouse click on rows
- Wikilinks `[[name]]`, `[[path|label]]` resolved under `foam/` (by file stem, then path); Enter opens; back key returns (history stack)
- ADR-012 amendment (wikilinks now in scope)

### Out of scope

- Raw/rendered toggle (#318)
- Editing; graph view; other roots

## Technical approach

`crates/cdamux-core/src/foam.rs`: build a tree from `list_foam`, wikilink extraction + resolution. `crates/cdamux-cli/src/fleet_draw.rs` / `tui.rs`: draw tree rows, handle keys and `MouseEventKind::Down` (mouse capture already enabled in `tui.rs`). Reuse `markdown.rs` for preview.

## Tasks

- [ ] Tree model + tests (ordering, collapse state, caps)
- [ ] Draw + keys + mouse (snapshot)
- [ ] Wikilink resolution + history + tests
- [ ] ADR-012 amendment

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Mouse events differ in popup vs pane | Test both paths; keys always work |
| Ambiguous wikilink stems | Prefer exact path, then first match; show unresolved in dim |

## Test plan

- [ ] Unit: tree build, toggle, wikilink resolve
- [ ] Snapshot: tree with expanded/collapsed dirs

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
