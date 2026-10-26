# PLAN-43 — cdamux send without attach

- **Status**: in-progress
- **PRD**: [[../prd/PRD-39-cdamux|PRD-39]] (research v0.1)
- **GitHub**: [#43](https://github.com/chatondearu/ai-dev-kit/issues/43)
- **Date**: 2026-10-26

## Summary

Add `cdamux send` to push text/keys to a session pane without attaching (ccmux /
agent-manager parity).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux remains execution surface |

## Scope

### In scope

- CLI subcommand; tmux `send-keys`
- Optional `--enter` flag

### Out of scope

- HTTP send (#44); dashboard UI for send (#42)

## Technical approach

Resolve target session → default window/pane; build `tmux send-keys` argv in core
for reuse by daemon HTTP later.

## Tasks

- [x] CLI: `send <session> <message...>` with documented pane targeting rule
- [x] Flag `--enter` appends Enter after message
- [x] Core helper callable from daemon (no duplicate tmux logic in HTTP yet)
- [x] Document in `cdamux/docs/dogfood.md`

## Test plan

- **Seams**: argv builder unit tests; invalid session → clear error

## Verification

- [x] `cargo test`; manual send to live tmux session
