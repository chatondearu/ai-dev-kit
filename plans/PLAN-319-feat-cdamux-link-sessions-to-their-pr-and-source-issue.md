# PLAN-319 — feat(cdamux): link sessions to their PR and source issue

- **Status**: ready
- **PRD**: [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]]
- **GitHub**: [#319](https://github.com/chatondearu/ai-dev-kit/issues/319)
- **Date**: 2026-11-14

## Summary

Each agent/session knows its PR and source issue: auto-detected and cached, `orch.sh` workers record theirs, and `cdamux link` overrides.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Store in `agents/<id>.json` via the state API (single write path) |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | No network on refresh paths |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Fields in agent state: `pr` {number,url,state,title,checked_at}, `issue` {number,url}, `link_override`
- Detection: branch → `gh pr list --head <branch> --json number,url,state,title,body`; issue from `Closes|Fixes #N` in body, else leading number in branch (`feat/177-…`, `worker/…/295-…`)
- TTL cache (e.g. 10 min), refreshed off the draw path
- `orch.sh`: write `pr` / `issue` when a worker opens its PR (it knows both)
- `cdamux link <session|agent-id> [--pr N|URL] [--issue N] [--clear]`

### Out of scope

- Display (#320)
- GitHub writes

## Technical approach

New `crates/cdamux-core/src/links.rs` (parse + detect + cache), CLI subcommand in `main.rs`, `orch.sh` hook in `skills/core/cda-dev/scripts/lib/cdamux_state.sh`. `gh` absent/offline → no link, no error.

## Tasks

- [x] Parsing (branch number, Closes/Fixes) + tests
- [x] Detection with injectable `gh` runner + cache TTL tests
- [x] `cdamux link` + override precedence test
- [x] orch.sh writes pr/issue + shell test

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| `gh` latency/rate | Cache + background refresh, never on draw |
| Detached HEAD / no branch | No link |

## Test plan

- [x] Unit: parse, precedence, TTL; shell: orch.sh writes fields

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
