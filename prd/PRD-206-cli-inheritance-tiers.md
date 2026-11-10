# PRD-206 — Workers inherit the parent CLI and run at model/effort tiers

- **Status**: shipped
- **GitHub**: [#206](https://github.com/chatondearu/ai-dev-kit/issues/206)
- **Date**: 2026-11-09
- **Tags**: #product #orchestration #cost

## Problem

`orch.sh` picks the first agent CLI on `PATH` (`agent`, `cursor-agent`, `claude`,
`opencode`), so a run started from a Claude Code session can spawn Cursor workers
(observed on the maintainer's Mac). Every role (worker, reviewer, verifier,
integrator, pr-fix) then runs the CLI's default model and effort: no cost control,
and no way to spend more only where a task actually fails.

## Goals

- [x] Workers use the CLI of the session that launched the orchestrator, unless
      the user says otherwise in that session.
- [x] Each role runs at a cost-bounded tier; a tier is a CLI-specific model/effort.
- [x] A failing worker escalates one tier per failure, bounded by a ceiling.
- [x] The operator sees which CLI/model/effort/tier each worker used.

## Non-goals

- Learned router, difficulty classifier, difficulty labels on issues.
- Anthropic advisor tool (API only).
- Automatic cross-vendor verifier (opt-in only).
- Installing CLIs ([[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]]).

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer in Claude Code | `/cda-dev` → `orch.sh` → workers run `claude -p --model sonnet --effort high` |
| Maintainer in Cursor | Same run → `agent -p --force --model auto` |
| Maintainer wants a second opinion | "verifier en codex" → session sets `ORCH_AGENT_VERIFIER=codex` |
| AFK host | `orch-auto` timer without `ORCH_AGENT` → clear failure, no silent CLI choice |
| Cost-wary run | `ORCH_TIER_MAX=medium` → never reaches `large` |

## Requirements

### Functional

- [x] #207 — CLI resolution `ORCH_AGENT_<ROLE>` > `ORCH_AGENT` > parent CLI (env
      markers, else `ps` ancestry) > PATH; `orch-auto` requires `ORCH_AGENT`;
      SKILL.md override translation; ADR-010.
- [x] #208 — `config/tiers.tsv`, `ORCH_TIERS_FILE`, `ORCH_TIER_<ROLE>`, role
      defaults (integrator `small`, others `medium`), explicit `--model` bypass,
      unknown-CLI warning, `cli/model/effort/tier` in status.json and graph nodes.
- [x] #209 — one tier up per failure in worker and pr-fix loops up to
      `ORCH_TIER_MAX` (default `large`), then `blocked`; graph-plan task `tier`.
- [x] #210 — doctor warns on Cursor ids missing from `--list-models`.
- [x] #211 — cdamux Fleet shows cli/model/tier.

### Non-functional

- [x] Tests use stub CLIs and temp dirs; never the live config.
- [x] Default behaviour with `ORCH_AGENT` set and no tier env stays compatible
      except for the added tier flags.
- [x] `nix develop -c bash scripts/verify.sh` green.

## Tier table (initial)

| Tier | claude | cursor |
| ---- | ------ | ------ |
| small | `--model sonnet --effort low` | `--model composer-2.5` |
| medium | `--model sonnet --effort high` | `--model auto` |
| large | `--model opus --effort high` | `--model claude-opus-5-5-high` |

Cursor `auto` in medium is an accepted economy trade-off: small → medium may not
change the model.

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]],
  [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]], ADR-010 (new, #207)

## Open questions

1. Cursor and opencode env markers in agent shells are unknown → `ps` ancestry fallback.
2. opencode / codex tier rows are not run-verified on the maintainer host.

## Success metrics

- From a Claude session, `orch.sh --dry-run` reports `claude` for every role.
- A red verify re-runs one tier up and ends `blocked` at the ceiling.

## Implementation pointer

`foam/archive/plans/PLAN-207-worker-cli-inheritance.md`, `foam/archive/plans/PLAN-208-model-tiers.md`,
`foam/archive/plans/PLAN-209-tier-escalation.md`, `foam/archive/plans/PLAN-210-tiers-doctor.md`,
`foam/archive/plans/PLAN-211-cdamux-tier.md`
