# PLAN-26 — Exact Status option match and stub tests

- **Status**: done
- **PRD**: [[../../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- **GitHub**: [#26](https://github.com/chatondearu/ai-dev-kit/issues/26)
- **Date**: 2026-10-20

## Summary

Replace soft `contains` Status matching with case-insensitive exact match only; add stub tests so Abandoned/Already Reviewed cannot be selected for Done/Ready.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Board helper stays kit-side; no host installs |
| [[../../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | list-ready must keep feeding orch-auto correctly |

## Scope

### In scope

- `closest_status_option` exact-only behavior + clear die message
- Automated tests under `skills/core/cda-kanban/`

### Out of scope

- Reconcile GraphQL rewrite

## Technical approach

Edit `gh-board.sh` soft-match branch; extract or exercise `closest_status_option` via a small `scripts/tests/` harness with fixture JSON.

## Tasks

- [ ] Exact-only match; error lists available option names when missing
- [ ] Tests: Ready ≠ Already Reviewed; Done ≠ Abandoned; exact Ready/Done still resolve
- [ ] Smoke: `bash -n` on gh-board.sh

## Test plan

- [ ] New test script exits 0
- [ ] Manual: `closest` behavior via test fixtures

## Verification

- [ ] Lint/test per AGENTS.md relevant suite
