# PLAN-30 — Frontier fail-closed when body fetch fails

- **Status**: in-progress
- **PRD**: [[../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- **GitHub**: [#30](https://github.com/chatondearu/ai-dev-kit/issues/30)
- **Date**: 2026-10-20

## Summary

Hold an issue when `gh` cannot load its body instead of treating missing blockers as empty.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | Frontier gating must not silently clear dependencies |

## Scope

### In scope

- `orch_frontier_partition` fail-closed on body fetch error
- `test_frontier.sh`

### Out of scope

- Changing `orch_issue_state` fail-closed (already OPEN on error)

## Tasks

- [ ] Distinguish fetch failure from empty body; hold on failure
- [ ] Update comments to match
- [ ] Test with stub gh that fails body fetch

## Test plan

- [ ] `test_frontier.sh` green

## Verification

- [ ] `bash skills/core/cda-dev/scripts/tests/test_frontier.sh`
