# PLAN-354 — fix(cda-dev): harden orch_export_functions for shells without compgen

- **Status**: done
- **PRD**: [[../prd/PRD-353-harden-orch-export-without-compgen|PRD-353]]
- **GitHub**: [#354](https://github.com/chatondearu/ai-dev-kit/issues/354)
- **Date**: 2026-11-15

## Summary

Stop relying on `compgen` to discover harness functions for `export -f`; always
use `declare -F`. Assert that `orch_loop_worker` is visible to a child `bash -c`
before sandbox worker invoke. Add a suite self-check and a short note in
`doc/orchestration.md`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]] | Keep worker loop / independent review / fail-closed verify intent |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Graph worker path must keep working after the same export fix |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Harness stays portable bash; no new install.sh wiring |

## Scope

### In scope

- `orch_export_functions` discovery via `declare -F` only
- Loud assert before worker `bash -c` (ready-pickup / graph / headless / pr-fix paths that use export + sandbox bash -c)
- Self-check in cda-dev script tests
- Short doc note in `doc/orchestration.md`

### Out of scope

- Sourced worker entrypoint (remove `export -f`)
- Motoko known-env deep dive
- Coolify work / unrelated dirty tree files

## Technical approach

1. Rewrite `orch_export_functions` to iterate `declare -F` and `export -f` for
   `orch_*` / `_orch_*` only — no `compgen`.
2. Add a small helper (e.g. `orch_assert_exported`) that runs
   `bash -c 'type orch_loop_worker'` (and optionally `orch_graph_worker`) after
   export; fail with a clear stderr message on miss.
3. Call the assert once after `orch_export_functions` at the harness entry that
   already exports (and/or immediately before each sandbox worker `bash -c` if
   that is clearer and still KISS — prefer one call site if sufficient).
4. Extend an existing loop/export test (or add a focused case) that sources the
   libs, calls `orch_export_functions`, then asserts child bash can `type`
   `orch_loop_worker`.
5. One short subsection in `doc/orchestration.md`: export/assert requirement;
   if worker invoke still fails, skill-only fallback / `cda-graph` `graph-node.sh`
   + Cursor Task.

Local Motoko / uncommitted WIP that preferred-compgen-then-fallback should be
replaced by declare-F-only (intake Q2=A).

## Tasks

- [x] **Export discovery**: `orch_export_functions` uses only `declare -F`; no
      `compgen` (acceptance: grepping the function body finds no `compgen`)
- [x] **Assert**: after export, child `bash -c 'type orch_loop_worker'` must
      succeed or harness exits with a clear error naming export failure
- [x] **Call sites**: ready-pickup / graph-plan / headless / pr-fix paths that
      rely on exported functions get the assert (or share one post-export assert)
- [x] **Test**: suite case covers export → `bash -c 'type orch_loop_worker'`
- [x] **Docs**: short note in `doc/orchestration.md` (export/assert + Task /
      `graph-node.sh` fallback)
- [x] **Verify**: `bash scripts/verify.sh` green; PR `Closes #354`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| `export -f` still broken under some bwrap | Assert fails loud; follow-up entrypoint issue if reproduced |
| Assert too late / only one mode fixed | Grep all `bash -c` + `orch_*_worker` call sites; share helper |
| Dirty main tree (Coolify WIP) | Worktree from HEAD; do not commit unrelated files |

## Test plan

Seams: `orch_export_functions` public behavior; child bash visibility of
`orch_loop_worker` after export.

- [x] Unit/self-check: source libs → export → `bash -c 'type orch_loop_worker'` → 0
- [ ] Optional negative: simulate empty export path only if cheap (skip if awkward)
- [x] `bash scripts/verify.sh` (host / Mise; no routine `nix develop -c`)

## Verification

- [x] Lint / test / build per project conventions
- [x] No new ADR unless we later kill `export -f` (out of scope here)
