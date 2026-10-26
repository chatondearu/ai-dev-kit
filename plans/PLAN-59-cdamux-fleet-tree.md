# PLAN-59 — cdamux fleet tree (v2 phase 1)

- **Status**: in-progress
- **PRD**: [[../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- **GitHub**: [#59](https://github.com/chatondearu/ai-dev-kit/issues/59)
- **Parent**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Stack**: PR base `main`

## Summary

Project→session expandable tree; status icons; attach on Enter; `--pick` unchanged.

## Tasks

- [x] Core: `ProjectSessionTree` from discover + tmux sessions (unassigned bucket)
- [x] Core: unit tests path→project matching
- [x] TUI: tree render (project color vs session indent + ○/●/◆)
- [x] TUI: expand/collapse, j/k, Enter attach, q quit
- [ ] Docs: architecture + dogfood tree scenario

## Verification

`cd cdamux && cargo test`
