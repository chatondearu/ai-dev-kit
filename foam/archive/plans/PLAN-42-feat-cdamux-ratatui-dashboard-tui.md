# PLAN-42 — cdamux ratatui dashboard TUI

- **Status**: done
- **PRD**: [[../../prd/PRD-39-cdamux|PRD-39]]
- **GitHub**: [#42](https://github.com/chatondearu/ai-dev-kit/issues/42)
- **Date**: 2026-10-26

## Summary

Default `cdamux` launches a ratatui dashboard for grouped sessions, attach, and
spawn entry; `cdamux --pick` remains skim-only for tmux popups.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-004-cdamux|ADR-004]] | CLI + optional daemon; no telemetry |
| [[../../foam/decisions/ADR-003-cda-aitools|ADR-003]] | No aitools enable |

## Scope

### In scope

- Full-screen TUI when no subcommand
- Reuse list grouping and attach/spawn from core/CLI

### Out of scope

- skzv Notes/Conversations tabs; hunk review (#46)

## Technical approach

Add `ratatui` + `crossterm` to `cdamux-cli`; thin event loop: render grouped list,
handle keys for attach/quit/spawn. Delegate spawn to existing spawn module.

## Tasks

- [x] Tracer: render static session list from `grouped_for_list` with config `group_by`
- [x] Keybind: attach/switch selected session (nested tmux → `switch-client`)
- [x] Keybind: quit; keybind: open spawn flow (reuse `-i` or inline skim for v1 slice)
- [x] Ensure `cdamux --pick` code path unchanged (skim, exit 0)
- [x] Terminal cleanup on panic/quit (alternate screen)

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| TUI in tmux popup | Document: use `--pick` in popups, dashboard in full terminal |

## Test plan

- **Seams**: pure functions for key → action mapping; smoke test `cargo test` with TUI module behind feature flag optional
- [x] Manual dogfood checklist row for dashboard

## Verification

- [x] `cargo build && cargo test` in `cdamux/`
- [x] Manual: `cdamux` in tmux and outside (see `dogfood.md` sign-off)
