# PLAN-47 — cdamux notes markdown per project

- **Status**: done
- **PRD**: [[../../prd/PRD-39-cdamux|PRD-39]] (v1)
- **GitHub**: [#47](https://github.com/chatondearu/ai-dev-kit/issues/47)
- **Date**: 2026-10-26

## Summary

Per-project markdown notes viewable from cdamux (skzv Notes / Teep inspiration).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-004-cdamux|ADR-004]] | Local files only; no cloud sync v1 |

## Scope

### In scope

- Config or convention for notes path
- TUI view; external editor hook acceptable

### Out of scope

- Full Teep sidecar

## Technical approach

Config key e.g. `notes.root` or `<project>/.cdamux/notes.md`; dashboard panel or
subcommand `notes`.

## Tasks

- [x] Config schema + example TOML
- [x] List/open note for project tied to selected session
- [x] Optional terminal markdown render (dependency justified in PR)
- [x] Architecture/dogfood doc update

## Test plan

- Config load tests; path resolution unit tests

## Verification

- [x] Manual create note, open from TUI (merged via PR #55)
