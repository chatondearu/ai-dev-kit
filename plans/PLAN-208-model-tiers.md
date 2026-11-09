# PLAN-208 — Run each role at a model/effort tier

- **Status**: ready
- **PRD**: [[../prd/PRD-206-cli-inheritance-tiers|PRD-206]]
- **GitHub**: [#208](https://github.com/chatondearu/ai-dev-kit/issues/208)
- **Date**: 2026-11-09
- **Blocked by**: #207

## Summary

Map `(cli, tier)` to CLI flags from one TSV table; each role has a default tier;
the chosen cli/model/effort/tier is visible to operators.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR-010 (#207) | CLI resolution order is fixed; tiers only add flags |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Graph node schema stays backward compatible |

## Scope

### In scope

- `skills/core/cda-dev/config/tiers.tsv` (`cli<TAB>tier<TAB>flags`, `#` comments):
  - claude: small `--model sonnet --effort low`, medium `--model sonnet --effort high`, large `--model opus --effort high`
  - cursor (`agent`/`cursor-agent`): `--model composer-2.5`, `--model auto`, `--model claude-opus-5-5-high`
  - opencode (`-m provider/model#variant`) and codex (`-m … -c model_reasoning_effort=…`) rows from their docs, commented "unverified"
- `lib/agents.sh`: `orch_tier_for_role <role>` (`ORCH_TIER_<ROLE>`, else defaults:
  integrator `small`, others `medium`; `ORCH_TIER` env read as the current tier
  when set by #209), `orch_tier_flags <cli> <tier>`; `_orch_agent_exec` appends flags.
- Bypass: the resolved agent command already contains `--model` → no tier flags, tier recorded as `pinned`.
- CLI with no table row → no flags, one warning per run (`ORCH_RUN_ID`-scoped marker).
- `ORCH_TIERS_FILE` replaces the kit table (missing file → error, not silent).
- Record `cli`, `model`, `effort`, `tier` per worker in `.orch/run/status.json`
  (`lib/progress.sh`) and graph nodes (`lib/graph.sh`, `cda-graph/templates/orch-graph.schema.json`).
- Append "Tiers" section to ADR-010; docs `doc/configuration.md`, `doc/orchestration.md`, `doc/graph-orchestration.md`.

### Out of scope

- Escalation (#209), doctor (#210), cdamux display (#211).

## Tasks

- [ ] tiers.tsv + parser (bash, no deps)
- [ ] flags in `_orch_agent_exec` for claude / cursor / opencode / codex
- [ ] bypass + unknown-CLI warning
- [ ] status.json + graph node fields + schema
- [ ] dry-run prints tier and flags per role
- [ ] docs + ADR-010 section

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| `CLAUDE_EFFORT` inherited from the parent session | explicit `--effort` always passed for claude |
| `agent -p --model auto` rejected headless | dry-run note; if rejected, fall back to no flag for that tier and document |

## Test plan

- [ ] `tests/test_agents.sh` (or new `test_tiers.sh`): flags per cli/tier via a recording stub; role defaults; `ORCH_TIER_<ROLE>`; `ORCH_TIERS_FILE`; `--model` bypass; unknown CLI warns once
- [ ] status.json contains the four fields (`test_progress.sh`)
- [ ] `nix develop -c bash scripts/verify.sh`
