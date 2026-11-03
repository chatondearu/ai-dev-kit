# PRD-129 — Fix kiss-audit 2026-11-03 (harness gates, untrusted input, cdamux targeting)

- **Status**: shipped
- **GitHub**: [#129](https://github.com/chatondearu/ai-dev-kit/issues/129)
- **Date**: 2026-11-03
- **Tags**: #quality #tooling #security #architecture

## Problem

The 2026-11-03 `/cda-kiss-audit` (on `main` `5bd1522`, right after PRD-90 shipped)
found 20 ranked findings and 19 smaller ones, each with a concrete repro. The
gates PRD-90 added do not gate: the ADR-006 verifier passes on a stale
`verifier-ok`, `pr-fix` refuses every verify command (missing `export -f`),
`orch: allow-no-checks` never matches real `gh` output, and `--no-graph` skips the
verifier. Untrusted GitHub input reaches an agent with full permissions (any
commenter's `## Agent Brief`; fork PRs pushed to `origin HEAD:main`).
`setup-agents.sh --init-foam --force` deletes `prd/`, `plans/` and `.cursor/`.
`scripts/verify.sh` is red on the operator's Mac. In cdamux, `-t <name>` hits the
wrong session, the tailnet server reads unbounded bodies before auth, and the TUI
acts on a row index that moves under the user.

## Goals

- [x] Every repro of the audit no longer reproduces; non-trivial fixes carry one test
- [x] `nix develop -c bash scripts/verify.sh` exits 0 on darwin (cargo test included)
- [x] No GitHub input from a non-collaborator becomes agent instructions or a push target
- [x] Delivered by parallel Task subagents (one PR per child); operator reviews and merges

## Non-goals

- Auto-merge
- New orch features or modes
- Rewriting the cdamux TUI; #127 (preview sidecar spawns) stays separate
- Deleting `archive/`

## Decisions taken at intake (operator, 2026-11-03)

1. Scope = the 20 ranked findings + the 19 smaller ones.
2. Delete `foam-graph-export.sh` (broken, unread) instead of fixing it.
3. Tailnet token stays optional; POST must be `application/json` and `Host` must
   match the bind address.
4. `pr-fix` on a cross-repo PR, or on a head branch equal to the default branch → `blocked`.
5. `flake.nix` gains `devShells.default`; `nix develop -c bash scripts/verify.sh`
   is the documented gate.

## Requirements

See [[../foam/archive/plans/PLAN-129-kiss-audit-2026-11-03|PLAN-129]] — one child issue per task
(#130–#140), each with the finding, repro, smallest fix and verify command.

## Success metrics

- All 11 child PRs merged; `bash scripts/verify.sh` green on `main` in the devShell
- `cargo test` green, no new warnings

## Links

- Plan: [[../foam/archive/plans/PLAN-129-kiss-audit-2026-11-03|PLAN-129]]
- Prior batches: [[PRD-90-kiss-audit-2026-11|PRD-90]], [[PRD-25-kiss-audit-orch-gh-board|PRD-25]], [[PRD-6-kiss-audit-fixes|PRD-6]]
- ADRs: [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[../foam/decisions/ADR-004-cdamux|ADR-004]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]],
  [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]]
