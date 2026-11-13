# PLAN-316 — test(cdamux): e2e orchestrator scenario on throwaway tmux + live dogfood checklist

- **Status**: ready
- **PRD**: [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]]
- **GitHub**: [#316](https://github.com/chatondearu/ai-dev-kit/issues/316)
- **Date**: 2026-11-14

## Summary

Prove the single-session orchestration loop end to end with an automated scenario, and give the maintainer a ≤ 15 min live checklist. Gaps become follow-up issues.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Statuses, inbox, delivery rules are the contract under test |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local only; throwaway tmux |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Integration test: `orch new` → `worker spawn --prompt-file` → `ask` → `answer` → `done` → orchestrator digest/nudge
- Assertions: statuses (`blocked` set/cleared, `done`), `✉N` / `?`, delivery word (`pasted|nudged|queued`), spec intact in the worker's first prompt
- `cdamux/docs/dogfood.md`: live checklist (real Claude orchestrator + 2 workers)
- Follow-up issues for gaps

### Out of scope

- Product fixes (unless trivial, < 20 lines)
- Nudge policy (#278)

## Technical approach

New `crates/cdamux-core/tests/orch_loop_tmux.rs` reusing helpers of `worker_tmux.rs` / `msg_tmux.rs` / `orch_tmux.rs` (throwaway `-L` server, temp `CDAMUX_HOME`, stub `claude` that prints its args and stays alive). Drive the loop with the core API or the built CLI; read state with `state_get`, badges with `inbox_badges`.

## Tasks

- [ ] Scenario test, one assertion per hop, with a clear failure message per hop
- [ ] Dogfood checklist section (spawn, relay, ask/answer, done, review on `gh pr diff`)
- [ ] File follow-up issues for any failing hop (`area:cdamux`, Parent #315)

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Flaky timing (nudge quiet window) | Inject a short quiet window or poll with timeout |
| Stub client unlike real ones | Live checklist covers real clients |

## Test plan

- [ ] Scenario passes; each hop fails with a named message when broken

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
