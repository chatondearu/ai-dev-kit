# PLAN-360 — fix(cda-orchestrator): hard rules, planner worker, exit-75 fallback, end turn after dispatch

- **Status**: ready
- **PRD**: [[../prd/PRD-356-orchestrator-actually-orchestrates|PRD-356]]
- **GitHub**: [#360](https://github.com/chatondearu/ai-dev-kit/issues/360)
- **Date**: 2026-11-15

## Summary

Make `cda-orchestrator` a dispatcher: hard rules, a planner worker for issue/PRD/plan
creation, a worker fallback when `orch.sh` exits 75, and turn discipline.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | Fast-lane brief is the small-work path |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Messages only via `cdamux msg`; nudge wakes an idle orchestrator |
| [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]] | Spawned workers still verify + review before `done` |

## Scope

### In scope

- `skills/core/cda-orchestrator/SKILL.md`
- `skills/core/cda-dev/prompts/intake.md` § On GO (orchestrator variant)
- `cdamux/prompts/orch-boot.md` only if a one-line pointer to the hard rules helps

### Out of scope

- Hook routing (#359), nudge policy (#278)

## Technical approach

1. `## Hard rules` at the top: never edit, commit, or run verify/tests in a repo; never
   `gh pr checkout`; every change goes to a worker; quick lookups only.
2. Route table: chat request → one question round + GO here → planner worker spec
   (Goal: create issue(s), PRD, plan(s) per `cda-dev`; Report: issue numbers) or a
   fast-lane brief for small work → dispatch (`orch.sh` or `worker spawn`).
3. `orch.sh` exit 75 → `cdamux worker spawn <issue> --prompt-file` with the plan as spec.
4. `## Turn discipline`: dispatch then end the turn; no `sleep`, `AwaitShell` loops, or
   `--watch` in the foreground; wait for the `📨` nudge.
5. `intake.md` On GO: "inside a cdamux orchestrator, the planner worker does step 1".

## Tasks

- [ ] Hard rules section
- [ ] Planner worker lane + spec template
- [ ] Exit-75 fallback line
- [ ] Turn discipline section
- [ ] `intake.md` orchestrator variant
- [ ] `bash scripts/verify.sh`; PR `Closes #360`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Skill grows long | Keep each addition ≤ 6 lines; delete what the new rules replace |
| Planner worker creates duplicates | Spec says: search open issues first (cda-foam protocol) |

## Test plan

- [ ] `bash scripts/verify.sh` (skill lint / link checks)
