# PLAN-46 — cdamux hunk review → agent

- **Status**: done
- **PRD**: [[../prd/PRD-39-cdamux|PRD-39]] (v1)
- **GitHub**: [#46](https://github.com/chatondearu/ai-dev-kit/issues/46)
- **Date**: 2026-10-26

## Summary

TUI flow to inspect a git diff, select hunks, and send review instructions to the
agent session via the send pipeline (#43).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | v1 feature; depends on dashboard + send |

## Scope

### In scope

- Diff for session cwd; hunk selection; prompt formatting

### Out of scope

- Full agent-manager review UX; Teep file rail

## Technical approach

Use `git diff` subprocess; ratatui list of hunks; format message → `send` helper.

## Tasks

- [x] Resolve repo path from session metadata / pane cwd
- [x] Display hunks (file + header); toggle selection
- [x] Action sends composed prompt to bound session
- [x] Document keys in dogfood doc

## Test plan

- Unit: hunk parse from sample diff; send payload formatting

## Verification

- [x] Merged via PR #54; manual review on sample repo (maintainer)
