# PLAN-85 — agent tag on Fleet rows

- **Status**: done
- **Issue**: #85
- **PRD**: `prd/PRD-83-cdamux-fleet-status-preview-agent.md`
- **Date**: 2026-10-28

## Constraints

| Source | Constraint |
| ------ | ---------- |
| ADR-004 | reuse existing `@cdamux_agent` / chrome helper patterns |
| PRD-72 | chrome already shows agent; extend list only |
| PRD-83 | placeholder when option missing |

## Tasks

- [x] Surface agent label on Fleet session rows from tmux user options (same source as chrome)
- [x] Missing option → clear placeholder; no panic
- [x] Keep Preview chrome agent field behaviour compatible
- [x] Small unit or render-level check if a cheap seam exists
- [x] Run `cargo test` in `cdamux/`

## Verify

```bash
cd cdamux && cargo test
```
