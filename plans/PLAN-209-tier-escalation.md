# PLAN-209 — Escalate one tier per failure; graph-plan task tier

- **Status**: ready
- **PRD**: [[../prd/PRD-206-cli-inheritance-tiers|PRD-206]]
- **GitHub**: [#209](https://github.com/chatondearu/ai-dev-kit/issues/209)
- **Date**: 2026-11-09
- **Blocked by**: #208

## Summary

Spend more only on a concrete failure signal: each failed iteration re-runs one
tier up, bounded by `ORCH_TIER_MAX`; graph-plan tasks may set their start tier.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR-010 | Tiers from the table; pinned (`--model`) roles never escalate |
| [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]] | Verify stays fail-closed; reasons stay explicit |

## Scope

### In scope

- Worker loop (`lib/graph.sh` loop behind `orch_loop_worker`) and pr-fix loop
  (`orch.sh` pr-fix path): failure = red verify or failed grounding verifier.
  Next iteration exports `ORCH_TIER=<next>` (small → medium → large).
- `ORCH_TIER_MAX` (`small|medium|large`, default `large`). A failure at the
  ceiling ends the worker `blocked`, reason `tier ceiling <tier> reached: <last failure>`,
  even if `ORCH_MAX_ITER` is not exhausted.
- Pinned roles: today's behaviour (no escalation, `ORCH_MAX_ITER` only).
- `cda-graph/templates/graph-plan.schema.json` + `scripts/graph-plan-validate.sh`:
  optional `tier` enum per task; the task runner starts at it.
- status.json / graph node `tier` updated on every escalation.
- Docs: `doc/configuration.md` (`ORCH_TIER_MAX`), `doc/graph-orchestration.md`,
  `cda-graph/SKILL.md` flag table; `templates/graph-plan.example.json` shows `tier`.

### Out of scope

- Reviewer escalation (review gate keeps its one fix iteration).

## Tasks

- [ ] next-tier helper + ceiling check
- [ ] wire into worker loop and pr-fix loop
- [ ] graph-plan `tier` (schema, validator, runner)
- [ ] docs

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Escalating into `large` burns the subscription | at most one `large` run per worker by construction; `ORCH_TIER_MAX=medium` |
| Cursor `small → medium` may not change the model (`auto`) | accepted in PRD; documented |

## Test plan

- [ ] `tests/test_loop.sh`: stub agent + failing verify → tiers medium, large, then blocked with the ceiling reason; `ORCH_TIER_MAX=medium` → blocked after one failure; pinned role → no escalation
- [ ] validator accepts `tier`, rejects an unknown value
- [ ] `nix develop -c bash scripts/verify.sh`
