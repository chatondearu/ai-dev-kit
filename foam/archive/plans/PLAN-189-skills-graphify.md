# PLAN-189 — Query the graphify graph when CDA_GRAPHIFY=1

- **Status**: done
- **PRD**: [[../../prd/PRD-186-floorplan-graphify|PRD-186]]
- **GitHub**: [#189](https://github.com/chatondearu/ai-dev-kit/issues/189)
- **Date**: 2026-11-05

## Summary

When `CDA_GRAPHIFY=1` and `<repo>/graphify-out/graph.json` exists, kit skills
and the kiss SessionStart hook send agents to the graph before grep/file reads.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Skills only reference graphify; never install it |
| AGENTS.md gotcha | kiss hooks local-only, fail-open, fast |

## Scope

### In scope

- One shared rule file in `rules/` (+ reassembled CLAUDE.md source; Cursor rule if the kit ships rules there)
- One line in: cda-dev worker prompt, cda-review, cda-diagnose, cda-architecture, cda-codebase, cda-kiss-audit, cda-standards, cda-tdd, cda-plan, cda-explore; audit the rest
- kiss SessionStart hook: graph report pointer replaces repo map when on

### Out of scope

- `aitools/` (#188), `skills/tools/cda-floorplan/` (#187)

## Tasks

- [x] Rule file
- [x] Per-skill lines (audit list in PR body)
- [x] Hook change (Claude, Cursor SessionStart; opencode has no repo map)
- [x] Hook tests: on + graph, on + no graph, off

## Test plan

- [x] Hook tests in `skills/quality/cda-kiss/hooks/test.sh` (verify.sh entry)

## Verification

- [x] `nix develop -c bash scripts/verify.sh`
