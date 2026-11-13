# PLAN-269 — Deep-link foam paths into Foam tab

- **Status**: ready
- **PRD**: [[../prd/PRD-265-cdamux-foam-browser|PRD-265]]
- **GitHub**: [#269](https://github.com/chatondearu/ai-dev-kit/issues/269)
- **Blocked by**: #267
- **Date**: 2026-11-13

## Summary

When the active session/preview context yields a path under `foam/`, switch to
the Foam tab and focus that file (in-app only).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR from #266 | In-app open only for v0; foam/ scope |
| Depends on #267 browser | Selection / focus API must exist |

## Scope

### In scope

- Detect `foam/...` (absolute or repo-relative) from active context sources
  already available to Fleet (preview text, selected path, etc.)
- Focus Foam tab + select matching list entry
- Missing file → clear empty/error state
- Ignore non-foam paths

### Out of scope

- Opening `$EDITOR`
- Wikilink-only targets without a file path
- Indexing all historical session links

## Technical approach

Add a small path classifier (`is_foam_path` / normalize to foam-relative) and a
Fleet action “open in Foam”. Hook from the highest-signal existing context
(preview capture or path under cursor) without inventing a new link protocol.

## Tasks

- [ ] Normalize and validate paths under the anchored project’s `foam/`
- [ ] Action: set SideTab Foam + select file (or empty state if missing)
- [ ] Wire at least one context source (document which in PR)
- [ ] Tests: foam path focuses; non-foam ignored; missing file empty state

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| No reliable “link” source in TUI yet | Document chosen source; minimal hook beats speculative protocol |
| Path escapes foam via `..` | Reject after canonicalize |

## Test plan

- [ ] Unit: path normalize + escape rejection
- [ ] UI/integration or snapshot: deep-link selects expected row

## Verification

- [ ] `nix develop -c bash scripts/verify.sh`
- [ ] PR `Closes #269`
