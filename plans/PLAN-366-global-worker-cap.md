# PLAN-366 — feat(cdamux): global cap on live workers

- **Status**: ready
- **PRD**: [[../prd/PRD-357-global-orchestrator|PRD-357]]
- **GitHub**: [#366](https://github.com/chatondearu/ai-dev-kit/issues/366)
- **Date**: 2026-11-16

## Summary

`[orchestrator].max_workers` (default 6, `0` = no cap). `cdamux worker spawn` refuses
with a clear error when that many workers are already live across all projects.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] (proposed) | §4: one machine-wide cap in cdamux config |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Count from agent state, not from tmux alone |

## Scope

### In scope

- `cdamux/crates/cdamux-core/src/lib.rs` (`Config.orchestrator: OrchestratorConfig { max_workers: u32 }`, serde default 6)
- `cdamux/crates/cdamux-core/src/worker.rs` (count + refuse before creating the worktree;
  new `WorkerError` variant with count and cap)
- `cdamux/config/cdamux.example.toml` (`[orchestrator]` block, commented)
- `doc/configuration.md` (one row)

### Out of scope

- Per-project caps, queueing over the cap (caller retries or waits), `orch.sh`'s own
  `ORCH_MAX_PARALLEL`

## Technical approach

1. Live worker = agent with `role=worker`, `status` in `working`/`blocked`, and a `host`
   that is a live tmux session (reuse the existing live-host check used for workers).
2. In `spawn_worker`, before any side effect: if `max_workers > 0` and live ≥ cap →
   `Err(WorkerError::Cap { live, cap })`, message
   `worker cap reached: <live>/<cap> live workers ([orchestrator].max_workers)`.
3. Pure counting fn over agent values + a liveness predicate, so it unit-tests without tmux.

## Tasks

- [ ] Config struct + default + example toml + doc row
- [ ] Count fn (dormant/done/dead-host excluded) + unit tests
- [ ] Refuse in `spawn_worker` before worktree creation; CLI exits non-zero
- [ ] `cargo test` in `cdamux/`; `bash scripts/verify.sh`; PR `Closes #366`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Stale `working` records block spawns | Dead tmux host never counts |
| Existing configs without the block | serde default; schema_version unchanged |

## Test plan

- [ ] Unit: count with working/blocked/done/dormant/dead-host mixes; cap 0 = unlimited
- [ ] `bash scripts/verify.sh`
