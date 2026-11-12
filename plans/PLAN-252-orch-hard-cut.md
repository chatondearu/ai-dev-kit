# PLAN-252 — Hard-cut orch status/graph to CDAMUX_HOME

- **Status**: implemented
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#252](https://github.com/chatondearu/ai-dev-kit/issues/252)
- **Blocked by**: #251
- **Date**: 2026-11-12

## Summary

Hard cut orch run visibility to `$CDAMUX_HOME/runs/`: remove writers to
`.orch/run/status.json` and `.orch/graph/*`, point Fleet and `/v1/orch-*` at the
new root, default `ORCH_TMUX=1`, and close #236 by eliminating the leaky live-graph
paths.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §5 hard cut, no double-write period; artefacts stay in-repo |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Without cdamux, no Fleet visibility but pipeline runs |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | ORCH_TMUX workers; throwaway tmux in tests |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Graph UX remains; storage moves |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Remove status.json / graph live writers
- Fleet + API read `$CDAMUX_HOME/runs/`
- `ORCH_TMUX=1` default
- Close #236
- Migrate orch tests / docs

### Out of scope

- Parent completion msgs (#253)
- Publish plumbing itself (#251 — blocker)
- Fleet orchestrator glyphs / unread (slice 4)

## Technical approach

After #251 publish path exists, delete or no-op the old writers, retarget readers
in cdamux refresh/API, flip default env, and rewrite tests that asserted on
`status.json` paths.

## Tasks

- [x] **Stop writers**: no code under `skills/` / `cdamux/` writes `.orch/run/status.json` or `.orch/graph/live|runs|current.json` (acceptance: `rg` clean for writers; tests fail if reintroduced)
- [x] **Readers**: Fleet and `/v1/orch-graph|orch-status|orch-runs` read `$CDAMUX_HOME/runs/` (acceptance: unit/API tests with temp home)
- [x] **ORCH_TMUX default**: `1` unless tests/`--sandbox` (acceptance: help/docs + test asserting default)
- [x] **#236**: pr-fix / graph-plan / pending mapping leaks resolved by new model (acceptance: issue checklist items covered or obsolete; close #236 via PR)
- [x] **Docs + verify**: update operator docs; `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Snapshot/Fleet tests tied to status.json fixtures | Update fixtures to CDAMUX_HOME runs |
| Users mid-run on old files | Hard cut per ADR; document one-release break |

## Test plan

Seams: (1) no old writers, (2) Fleet/API read new root, (3) default TMUX, (4) migrated orch tests.

- [x] `rg` gate or test for forbidden writer paths
- [x] Temp home Fleet/API
- [x] Full verify

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made (ADR-011 dual-write window closed)
