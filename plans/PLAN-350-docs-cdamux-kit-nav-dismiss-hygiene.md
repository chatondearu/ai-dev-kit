# PLAN-350 — docs: nav, dismiss, hygiene, sidebar deprecation

- **Status**: ready
- **PRD**: [[../prd/PRD-348-cdamux-opt-in-nav-bind-and-docs|PRD-348]]
- **GitHub**: [#350](https://github.com/chatondearu/ai-dev-kit/issues/350)
- **Blocked by**: #349
- **Date**: 2026-11-14

## Summary

Align kit and cdamux operator docs with shipped #334 and the #349 opt-in nav bind option names.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-014-cdamux-dedicated-navigator-session|ADR-014]] | Nav is dedicated session; sidebar deprecated |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Example/opt-in binds only |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `cdamux/docs/` (dogfood, architecture gaps)
- Kit `doc/` (orchestration, graph-orchestration, configuration as needed)
- `aitools/README.md` consistency with #349 flag names
- Remove sidebar-as-primary contradictions

### Out of scope

- Implementing the bind (#349)
- Changing product behavior

## Technical approach

Diff existing dogfood/architecture against PRD-334 keybinds; patch gaps; cross-link aitools navBind after #349 merges (rebase onto main).

## Tasks

- [ ] **cdamux docs**: nav keys, `d`, sweep-orphans, sidebar → nav (acceptance: dogfood checklist matches PRD-334)
- [ ] **kit doc/**: operator-facing mentions where Fleet/orch UX is described
- [ ] **aitools/README**: `navBind` / `--cdamux-nav-bind` with accurate names from #349
- [ ] **Contradiction pass**: no join-pane sidebar as primary path
- [ ] **Verify**: doc paths; optional `cda-doc-hygiene` if applicable

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Docs land before option names exist | Blocked by #349 |

## Test plan

- [ ] Manual read-through of linked pages
- [ ] Grep for “sidebar” primary guidance

## Verification

- [ ] Lint / test / build per project conventions if scripts touched
- [ ] Foam index PRD row if needed
