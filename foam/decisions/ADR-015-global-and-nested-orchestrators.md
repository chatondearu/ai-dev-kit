# ADR-015 — Global and nested cdamux orchestrators

- **Status**: accepted
- **Date**: 2026-11-15
- **Tags**: #architecture #tooling #cdamux #autonomy

## Context

[[ADR-011-cdamux-unified-state-model|ADR-011]] defines one orchestrator per long-lived
run, with workers under it via `parent`. The maintainer runs one orchestrator per project
and switches sessions to reach each. [[../../prd/PRD-357-global-orchestrator|PRD-357]]
asks for one global orchestrator able to dispatch into any discovered project, while
per-project orchestrators stay possible.

## Decision

1. **Scope** — an orchestrator agent carries `scope`: `project` (default, today's
   behaviour) or `global`. At most one live global orchestrator; its cwd is outside any repo.
2. **Target repo per spawn** — `cdamux worker spawn --repo <path|project>` picks the
   repo; without it, the caller's cwd repo (unchanged). Project names resolve through the
   same discovery as `cdamux projects`.
3. **Nesting** — an orchestrator may have a `parent` that is an orchestrator (depth ≤ 2:
   global → project → workers). Each orchestrator owns its run and its inbox; workers
   report to their direct parent only. A project orchestrator summarises to its parent
   with `cdamux msg` (`report` / `done` / `ask`).
4. **Capacity** — `[orchestrator].max_workers` caps live workers across all projects,
   checked at `worker spawn`.
5. **Role signal** — orchestrator panes export `CDA_ROLE=orchestrator`
   ([[../../prd/PRD-356-orchestrator-actually-orchestrates|PRD-356]]); the global boot
   prompt and hook routing build on it.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Global orchestrator talks directly to every worker of every project | One chat context grows with all projects; no place for per-project judgement |
| Message forwarding between project orchestrators (mesh) | More transport rules than ADR-011 §4 needs; tree is enough |
| Global orchestrator = Fleet UI action only (no agent) | Loses the conversational dispatch the maintainer wants |
| Unlimited depth | Restore ordering and inbox routing harder to reason about; no use case |

## Consequences

- **Positive**: one entry point for all projects; per-project orchestrators unchanged.
- **Negative**: one more state field (`scope`) and a cross-project worker count.
- **Constraint**: tests run on throwaway tmux servers; nothing enabled by `install.sh`.

## References

- [#357](https://github.com/chatondearu/ai-dev-kit/issues/357), children #363–#367
- `cdamux/crates/cdamux-core/src/worker.rs`, `orch.rs`, `restore.rs`
