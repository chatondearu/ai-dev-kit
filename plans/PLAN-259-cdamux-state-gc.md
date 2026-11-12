# PLAN-259 — Add retention GC for done/error agents and runs

- **Status**: draft
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#259](https://github.com/chatondearu/ai-dev-kit/issues/259)
- **Blocked by**: #258
- **Date**: 2026-11-12

## Summary

Retention policy: archive then delete `done`/`error` agents and runs with no host
older than 14 days (config.toml); keep `dormant` resumable agents; expose
`cdamux state gc`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §3 dormant reader-computed; state under CDAMUX_HOME |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Do not GC dormant resumable agents |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- 14-day default retention (configurable)
- Archive then delete done/error without host
- Keep dormant
- `cdamux state gc` manual entrypoint

### Out of scope

- Automatic systemd timer (manual gc is enough for v0 unless already patterned)
- Fleet UI (#257), restore (#258)

## Technical approach

Add config key + `state gc` subcommand that scans agents/runs, moves eligible
files to an archive dir then deletes after policy (or archive+delete in one
pass as documented).

## Tasks

- [ ] **Config**: retention days default 14 in config.toml / example (acceptance: parse test)
- [ ] **GC logic**: done/error + no host + age → archive then delete; dormant kept (acceptance: temp home unit tests with injectable clock/mtime)
- [ ] **CLI**: `cdamux state gc` (acceptance: CLI test)
- [ ] **Docs + verify**: document; `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Deleting too much | Dry criteria + tests; never touch dormant / live host |

## Test plan

Seams: (1) eligibility, (2) dormant keep, (3) CLI gc.

- [ ] Temp `CDAMUX_HOME` fixtures
- [ ] Full verify

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR only if a new decision was made
