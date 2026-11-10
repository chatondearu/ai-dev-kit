# PLAN-210 — Doctor warns on stale Cursor model ids

- **Status**: done
- **PRD**: [[../../prd/PRD-206-cli-inheritance-tiers|PRD-206]]
- **GitHub**: [#210](https://github.com/chatondearu/ai-dev-kit/issues/210)
- **Date**: 2026-11-09
- **Blocked by**: #208

## Summary

Cursor has no model aliases, so pinned ids in `tiers.tsv` rot. Doctor turns a
silent AFK worker failure into a readable warning.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Doctor checks, never installs |

## Scope

### In scope

- `scripts/doctor.sh`: when `agent` or `cursor-agent` is on PATH, run
  `--list-models` once and warn for each cursor `--model <id>` in the active
  table (`ORCH_TIERS_FILE` else the kit default) that is absent. `auto` is always valid.
- Warning only; exit status unchanged. Silent skip without a Cursor CLI or when
  `--list-models` fails (offline / logged out → one info line).

### Out of scope

- verify.sh network calls (verify stays offline).

## Tasks

- [x] check in doctor.sh
- [x] `scripts/tests/test_doctor.sh` case with a stub `cursor-agent`

## Test plan

- [x] stub lists a subset → warning names the missing id; full list → no warning; no CLI → no output
- [x] `nix develop -c bash scripts/verify.sh`
