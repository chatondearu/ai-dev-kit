# PRD-353 — Harden orch function export without compgen

- **Status**: approved
- **GitHub**: [#353](https://github.com/chatondearu/ai-dev-kit/issues/353)
- **Date**: 2026-11-15
- **Tags**: #product #tooling #orchestration

## Problem

On some agent shells (Cursor agent wrappers / constrained bash), `compgen` is
missing. `orch_export_functions` then never runs `export -f`, so
`orch_sandbox_wrap bash -c '… orch_loop_worker …'` (and `orch_graph_worker`)
fails with `command not found` after worktree and prompt setup already succeeded.

## Goals

- [ ] Discover harness functions without `compgen` (always `declare -F`)
- [ ] Fail loud before worker invoke if export did not expose `orch_loop_worker`
- [ ] Self-check test covers export → `bash -c 'type orch_loop_worker'`
- [ ] Short operator note: assert/export requirement + skill-only /
      `graph-node.sh` + Task fallback

## Non-goals

- Sourced worker entrypoint that removes `export -f` (follow-up if assert still
  fails under bwrap)
- Coolify / Motoko-ops product work
- Long Motoko/Nix “known environment” section

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Operator on NixOS + Cursor agent | `orch.sh --mode ready-pickup` runs the worker loop without `command not found` |
| Graph-plan operator | Parallel graph tasks invoke `orch_graph_worker` the same way |
| Operator when harness still breaks | Docs point to `graph-node.sh` + Cursor Task fallback |

## Requirements

### Functional

- [ ] `orch_export_functions` lists `orch_*` / `_orch_*` via `declare -F` only
- [ ] Shared assert (or equivalent) before sandbox `bash -c` worker invokes
      fails with a clear message if `orch_loop_worker` is not visible to a child bash
- [ ] Test in cda-dev harness suite covers the export self-check
- [ ] `doc/orchestration.md` notes the export/assert requirement and the
      skill-only / `graph-node.sh` + Task fallback

### Non-functional

- [ ] No change to ADR-006 review/verify gate intent
- [ ] Same export path for ready-pickup, graph-plan, headless, pr-fix
- [ ] `bash scripts/verify.sh` green

## Related project memory

- ADRs: [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]]
- Child: [#354](https://github.com/chatondearu/ai-dev-kit/issues/354)
- Plan: [[../plans/PLAN-354-fix-cda-dev-harden-orch-export-functions-for-shells-without-|PLAN-354]]

## Open questions

None — intake GO 2026-11-15 locked scope (harden + assert + test + short doc).

## Success metrics

- Motoko-style environment (no `compgen`) can export workers; assert catches
  remaining export failures before a silent `command not found`.

## Implementation pointer

[[../plans/PLAN-354-fix-cda-dev-harden-orch-export-functions-for-shells-without-|PLAN-354]]
