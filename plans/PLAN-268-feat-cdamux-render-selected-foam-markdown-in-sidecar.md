# PLAN-268 — Render selected foam markdown in sidecar

- **Status**: ready
- **PRD**: [[../prd/PRD-265-cdamux-foam-browser|PRD-265]]
- **GitHub**: [#268](https://github.com/chatondearu/ai-dev-kit/issues/268)
- **Blocked by**: #267
- **Date**: 2026-11-13

## Summary

When a foam markdown file is selected in the Foam browser, render a preview in
the sidecar using the existing pulldown-cmark path (today used by
`cdamux notes show --render`), with a bounded truncation policy.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR from #266 | Read-only; no wikilink resolution required in v0 |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local FS read only |

## Scope

### In scope

- Render selected foam `.md` in the Foam sidecar
- Truncation / size policy (replace silent 120-line plain dump with intentional
  preview limits)
- Tests for render + truncate

### Out of scope

- `[[wikilink]]` resolution
- HTML / mermaid / images
- Deep-link entry (#269) beyond consuming current selection

## Technical approach

Share or extract the CLI render helper into a core/UI-callable API; paint
rendered lines in the sidecar pane. Plain text fallback if render fails.

## Tasks

- [ ] Reuse pulldown-cmark render for a selected foam file path
- [ ] Define and test truncation (lines and/or bytes) with visible cue when
      truncated
- [ ] Wire selection from Foam list → preview pane
- [ ] Snapshot or unit coverage for rendered preview chrome

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Huge ADR freezes TUI | Hard truncate before parse/paint |
| Binary / non-utf8 | Skip with error line |

## Test plan

- [ ] Fixture markdown renders expected headings/lists subset
- [ ] Oversize file truncates without panic

## Verification

- [ ] `nix develop -c bash scripts/verify.sh`
- [ ] PR `Closes #268`
