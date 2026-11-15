# PLAN-361 — fix(cda-dev): isolate harness tests from live CDAMUX_HOME and agent id

- **Status**: done
- **PRD**: [[../prd/PRD-356-orchestrator-actually-orchestrates|PRD-356]]
- **GitHub**: [#361](https://github.com/chatondearu/ai-dev-kit/issues/361) · Refs [#273](https://github.com/chatondearu/ai-dev-kit/issues/273)
- **Date**: 2026-11-15

## Summary

`helpers.sh` exports a temp `CDAMUX_HOME` and unsets `CDAMUX_AGENT_ID`, so no harness test
writes into the live cdamux state or the live orchestrator inbox.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | State under `CDAMUX_HOME` only |

## Scope

### In scope

- `skills/core/cda-dev/scripts/tests/helpers.sh`
- One self-check (new small test or in an existing one)

### Out of scope

- #273 decisions; cleaning existing state

## Technical approach

1. In `helpers.sh`, after the `ORCH_*` scrub: `export CDAMUX_HOME="$(mktemp -d)"` and
   `unset CDAMUX_AGENT_ID`. Tests that set their own `CDAMUX_HOME` / id keep doing so.
2. Self-check: run one publishing test (e.g. `test_tmux.sh`) with `CDAMUX_HOME` set to an
   empty sentinel dir; assert the sentinel stays empty.

## Tasks

- [x] Export temp home + unset agent id
- [x] Sentinel self-check
- [x] `bash scripts/verify.sh`; PR `Closes #361`

## Test plan

- [x] Self-check passes; full suite green
