# PLAN-29 — Hard-fail sandbox and allowlist pr-fix verify

- **Status**: in-progress
- **PRD**: [[../prd/PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- **GitHub**: [#29](https://github.com/chatondearu/ai-dev-kit/issues/29)
- **Date**: 2026-10-20

## Summary

`--sandbox` without bwrap must exit 1; pr-fix must not `eval` free verify commands from the PR worktree.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Optional sandbox stays opt-in; fail closed when requested |

## Scope

### In scope

- `orch_sandbox_wrap` hard-fail
- Verify path for pr-fix: allowlist or `ORCH_VERIFY`
- Update SKILL.md / doc lines that say warn-and-continue
- Tests

### Out of scope

- Full bubblewrap policy redesign

## Tasks

- [ ] Missing bwrap + ORCH_SANDBOX=1 → exit 1
- [ ] pr-fix verify: allowlist (npm/pnpm/make/cargo/nix develop -c …) or ORCH_VERIFY; no free eval from PR AGENTS.md
- [ ] Docs/skill updated
- [ ] Tests for both behaviors

## Test plan

- [ ] test_sandbox + verify tests green

## Verification

- [ ] Relevant cda-dev tests pass
