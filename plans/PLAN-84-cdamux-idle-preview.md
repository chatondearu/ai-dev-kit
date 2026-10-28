# PLAN-84 — session idle + zero-history preview

- **Status**: done
- **Issue**: #84
- **PRD**: `prd/PRD-83-cdamux-fleet-status-preview-agent.md`
- **Date**: 2026-10-28

## Constraints

| Source | Constraint |
| ------ | ---------- |
| ADR-004 | cdamux in-tree; minimize deps; tmux subprocess OK |
| PRD-83 | No sub-agent UI parsing; fix idle + capture only |

## Tasks

- [x] Make empty/missing pane idle field fail closed (not Active-via-zero); add unit coverage for `1 \n` list-panes output
- [x] Choose fallback activity signal when idle unsupported; document one-line shortcut if heuristic
- [x] Fix pane preview capture so zero-history panes (Claude) return visible content (remove or correct `-E -1`)
- [x] Add regression covering capture args / empty-history behaviour at the seam available
- [x] Run `cargo test` in `cdamux/`

## Verify

```bash
cd cdamux && cargo test
```
