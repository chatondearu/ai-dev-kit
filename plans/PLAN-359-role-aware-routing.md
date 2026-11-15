# PLAN-359 — fix(cda-kiss,cda-agents): role-aware routing from one source

- **Status**: ready
- **PRD**: [[../prd/PRD-356-orchestrator-actually-orchestrates|PRD-356]]
- **GitHub**: [#359](https://github.com/chatondearu/ai-dev-kit/issues/359)
- **Date**: 2026-11-15

## Summary

`buildRouting` returns an orchestrator block when `CDA_ROLE=orchestrator`. The hook text
becomes the single source of the routing rule; other copies point to it.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | No new default install wiring |
| AGENTS.md gotchas | Kiss hooks local-only, fail-open, fast (no process spawn) |

## Scope

### In scope

- `skills/quality/cda-kiss/hooks/lib.js` (`ROUTING`, `buildRouting`)
- `skills/quality/cda-kiss/hooks/test.sh`
- Routing copies: `rules/01-project-workflow.md`, `skills/core/cda-agents/templates/AGENTS.base.md`,
  repo `AGENTS.md` (managed block) and `.cursor/rules/product-memory.mdc`

### Out of scope

- Spawn-side env (#358), `cda-orchestrator` skill content (#360)

## Technical approach

1. Add `ORCH_ROUTING` (≤ 6 lines): "CDA ROUTING — you are a cdamux orchestrator: never
   edit, commit or run verify/tests in a repo; every change goes to a worker (skill
   `cda-orchestrator`); one question round + GO, then dispatch and end the turn."
2. `buildRouting(env)`: `ORCH_RUN_ID` / `CDA_ROUTING=0` → `''` (unchanged precedence);
   `CDA_ROLE === 'orchestrator'` → `ORCH_ROUTING`; else `ROUTING`.
3. Pick one trivial-fix wording (hook's) and make the copies say the same thing or link to it
   in one line; regenerate the managed block from the template where the repo does so.

## Tasks

- [x] Orchestrator block + `buildRouting` branch
- [x] `test.sh`: default, orchestrator, silenced cases
- [x] Copies aligned (template, rules file, repo `AGENTS.md`, `.mdc`)
- [x] `bash scripts/verify.sh`; PR `Closes #359`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Managed `AGENTS.md` drifts from template | Edit template first, then sync the managed block |
| Cursor `beforeSubmitPrompt` context ignored (#237) | Session start is enough here; out of scope |

## Test plan

- [x] `bash skills/quality/cda-kiss/hooks/test.sh`
- [x] `bash scripts/verify.sh`
