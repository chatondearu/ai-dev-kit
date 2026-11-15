# PLAN-363 — feat(cdamux): worker spawn --repo for any discovered project

- **Status**: ready
- **PRD**: [[../prd/PRD-357-global-orchestrator|PRD-357]]
- **GitHub**: [#363](https://github.com/chatondearu/ai-dev-kit/issues/363)
- **Date**: 2026-11-16

## Summary

`cdamux worker spawn <name> --repo <path|project>` creates the worker's worktree, branch
and session in the target repo instead of the caller's cwd repo. Lets an orchestrator
outside any repo (global, #364) dispatch into every discovered project.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] (proposed) | §2: target repo per spawn; without `--repo`, unchanged behaviour |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | Worker `parent` stays the caller; run stays the caller's run |
| `cdamux/AGENTS.md` | Tests on a throwaway tmux server only |

## Scope

### In scope

- `cdamux/crates/cdamux-cli/src/main.rs` (`WorkerCommands::Spawn` gets `--repo`, `cmd_worker_spawn`)
- `cdamux/crates/cdamux-core/src/worker.rs` (`WorkerSpawnRequest.repo: Option<&Path>`;
  `spawn_worker` uses it instead of `resolve_caller_cwd` when set)
- Project resolution helper next to `discover_projects` (`cdamux-core/src/lib.rs`)
- Docs: `cdamux/README.md` or `doc/` CLI reference where `worker spawn` is listed;
  `skills/core/cda-orchestrator/SKILL.md` spawn block gets the flag (one line)

### Out of scope

- `orch new --global` (#364), worker cap (#366), nested orchestrators (#365)

## Technical approach

1. `--repo <value>`: if it is an existing directory → `git_toplevel` of it (error `NotGit`
   otherwise). Else treat as a project name: match `Project.name` from
   `discover_projects(&cfg.projects)`; exactly one match → its path; zero → error
   "unknown project"; several → error listing `root_label/name` candidates.
2. `spawn_worker`: `base_cwd = req.repo.map(PathBuf::from).unwrap_or_else(|| resolve_caller_cwd(..))`.
   `ensure_orch_run` keeps the caller's run (the run's `repo` stays the orchestrator's).
3. `--here` with `--repo` → spawn in that repo root without a worktree (same rule as today).
4. Worker state gets `repo` = target repo path so Fleet groups it under its project.

## Tasks

- [x] CLI flag + request field
- [x] Project name resolution (unique / unknown / ambiguous)
- [x] `spawn_worker` uses the target repo; works when caller cwd is not a git repo
- [x] Tests: unit for resolution; throwaway-tmux integration spawning into a second temp repo
      from a caller whose cwd is a non-git temp dir
- [x] Docs + skill line
- [x] `cargo test` in `cdamux/`; `bash scripts/verify.sh`; PR `Closes #363`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Project names collide across roots | Ambiguity error lists `root_label/name`; path always works |
| Worktree root shared across repos | Worktree dest already keyed by run + name; branch lives in the target repo |

## Test plan

- [x] `cargo test` (unit + `tests/worker_tmux.rs`-style throwaway server)
- [x] `bash scripts/verify.sh`
