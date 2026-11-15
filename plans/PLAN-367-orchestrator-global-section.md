# PLAN-367 — docs(cda-orchestrator): global section + accept ADR-015

- **Status**: done
- **PRD**: [[../prd/PRD-357-global-orchestrator|PRD-357]]
- **GitHub**: [#367](https://github.com/chatondearu/ai-dev-kit/issues/367)
- **Date**: 2026-11-16

## Summary

Add `cda-orchestrator` § Global (≤ 30 lines) for the orchestrator started by
`cdamux orch new --global` (#364), dispatching with `worker spawn --repo` (#363).
Accept ADR-015.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] | Becomes accepted; section must match its decisions |
| PLAN-360 hard rules | Never edit product code, never intake in the orchestrator, spawn a planner |

## Scope

### In scope

- `skills/core/cda-orchestrator/SKILL.md` (§ Global)
- `cdamux/prompts/orch-boot-global.md` only if it contradicts the section (keep ≤ 10 lines)
- `foam/decisions/ADR-015-*.md` status `accepted`; `foam/decisions/index.md`; `foam/index.md` row

### Out of scope

- Nested project orchestrators as children (#365): mention as "not yet" in one line;
  meanwhile heavy per-project work goes to `orch.sh --repo <path>` or a planner worker
  spawned with `--repo`

## Technical approach

Section content, in order:

1. You run outside any repo (`$CDAMUX_HOME/global`); list targets with `cdamux projects`.
2. Pick the repo per request; ask the human when the project is ambiguous — never guess.
3. Dispatch: `cdamux worker spawn <name> --repo <path|project> --prompt-file …`;
   several issues of one repo → `orch.sh --repo <path>`; exit 75 fallback as in hard rules.
4. Respect `[orchestrator].max_workers` (#366): on the cap error, wait or report — no retry loop.
5. Report per project (repo → workers → PRs), refs not pasted transcripts.

Verify command names against the merged code (`cdamux worker spawn --help`,
`cdamux orch new --help`) before writing.

## Tasks

- [x] § Global in the skill, consistent with hard rules (≤ 30 lines)
- [x] ADR-015 accepted + indexes
- [x] `bash scripts/verify.sh` (skill lint / length checks); PR `Closes #367`

## Test plan

- [x] `bash scripts/verify.sh`
