# PLAN-211 — cdamux shows cli, model and tier on worker rows

- **Status**: done
- **PRD**: [[../../prd/PRD-206-cli-inheritance-tiers|PRD-206]]
- **GitHub**: [#211](https://github.com/chatondearu/ai-dev-kit/issues/211)
- **Date**: 2026-11-09
- **Blocked by**: #208

## Summary

Operators see which CLI/model/tier each orch worker runs, and when it escalated.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | cdamux reads orch files, never writes them |

## Scope

### In scope

- `cdamux/crates/cdamux-core/src/orch_status.rs`: optional `cli`, `model`,
  `effort`, `tier` on `OrchWorkerStatus` (`#[serde(default)]`).
- Fleet `⚙` worker row / sidecar: compact `cli·tier` (model on the sidecar).
- Tailnet API `/v1/orch-status` passes the fields through.

### Out of scope

- Editing tiers from cdamux.

## Tasks

- [x] struct fields + serde default
- [x] TUI rendering
- [x] tests

## Test plan

- [x] unit test: old status file without the fields parses; new one exposes them
- [x] `nix develop -c bash scripts/verify.sh` (cargo tests)
