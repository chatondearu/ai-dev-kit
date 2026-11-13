# PLAN-297 — Optional fast Alt-P bind via cdamux --pick

- **Status**: ready
- **PRD**: [[../prd/PRD-293-cdamux-fast-popup-open|PRD-293]]
- **GitHub**: [#297](https://github.com/chatondearu/ai-dev-kit/issues/297)
- **Date**: 2026-11-13

## Summary

Document (and optionally template) an opt-in tmux bind using `cdamux --pick` for
instant switch-client; keep default `M-p` as full dashboard unless user opts in.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR-004 / PRD-41 | Mission Control remains the default Alt-P experience |

## Scope

### In scope

- Docs + kit tmux template comment/example for fast bind
- Dogfood checklist for both binds
- Default `M-p` unchanged

### Out of scope

- Changing default kit bind to `--pick`
- Perf of full dashboard (#294/#295)

## Technical approach

Update `aitools/templates/tmux.conf` and/or `cdamux/docs/dogfood.md` with a
commented alternative bind; do not flip production default.

## Tasks

- [ ] Document opt-in `--pick` bind vs full `cdamux`
- [ ] Dogfood checklist both paths
- [ ] Confirm default `M-p` still full dashboard

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Users confuse binds | Clear comments in template |

## Test plan

- [ ] Docs review; no behaviour change unless template explicitly uncommented

## Verification

- [ ] Verify green (docs-only OK)
- [ ] PR `Closes #297`
