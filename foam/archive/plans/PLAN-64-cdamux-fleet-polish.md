# PLAN-64 — Fleet polish + docs (v2 phase 6)

- **Status**: done
- **PRD**: [[../../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- **GitHub**: [#64](https://github.com/chatondearu/ai-dev-kit/issues/64)
- **Parent**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Stack**: PR base phase 5 branch

## Summary

Agent-manager keyboard parity on Fleet plus documentation closure for epic #58.

## Tasks

- [x] `w` waiting filter on tree
- [x] `F` expand all / fold all project nodes
- [x] `g` cycle `group_by` (tree regroups; documented vs AM)
- [x] `?` help overlay
- [x] Update `research.md`, `dogfood.md`, `prompts/next-agent.md`
- [x] PRD-41 → shipped (with maintainer sign-off on #58)

## Verification

`cd cdamux && cargo test`; manual Fleet keys; comment on #58 when dogfood OK.
