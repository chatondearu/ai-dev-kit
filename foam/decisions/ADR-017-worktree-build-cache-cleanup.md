# ADR-017 — Worktree build-cache cleanup and spawn disk guard

- **Status**: accepted
- **Date**: 2026-11-23
- **Tags**: #tooling #cdamux #disk

## Context

Every cdamux worker and every orch.sh issue gets its own git worktree. Build caches
stay there after the work is merged: one Rust worktree `target/` reached 9.7 GB, the
kit's `cdamux/target` about 2 GB each, and dozens of finished worktrees filled the
maintainer's disk twice in one week. Node projects pile up `node_modules`, `.nuxt` and
`.output` the same way. Removing whole worktrees or branches stays a human call
([[ADR-003-cda-aitools|ADR-003]] keeps orch worktrees local and manual).

## Decision

1. **Config** — `[worktrees].disposable` (default `["target", "node_modules", ".nuxt",
   ".output"]`) lists build dirs that may be deleted. An entry without `/` matches a
   directory name at any depth; with `/`, an exact path relative to the worktree.
   Absolute paths and `..` fail config parsing. `[]` turns cleanup off. One global list:
   the defaults cover Rust and Node, and only git-ignored dirs are touched, so a
   per-project list is not needed yet.
2. **Only git-ignored dirs** — candidates come from `git ls-files --others --ignored
   --exclude-standard --directory`; each must resolve inside the worktree and not be a
   symlink. Only worktrees cdamux or orch.sh created are scanned: linked (git dir ≠
   common dir, so never a main checkout or a submodule) and on a managed branch
   (`worker/…`, `feat/<id>-issue-<id>`, `orch/pr-<n>`; never a detached HEAD).
   Tracked or untracked work, the worktree and its branch are never removed.
3. **Finished for good** (pure `clean_verdict`): PR merged or closed (`gh pr list
   --head <branch> --state all`), or no PR, no live session and at least one commit of
   the branch's own (after its reflog creation point) already in the default branch —
   a branch with zero commits is never finished. Kept: a live tmux session rooted in
   the worktree whose agent is not `done`, an open PR, a PR state `gh` cannot read, or
   any failure to list tmux sessions (no server or another socket: fail closed).
4. **Entry points** — `cdamux worker clean [--dry-run] [--root DIR]` (manual, prints
   the size per worktree); a successful `cdamux worker spawn` starts it in the
   background (log `$CDAMUX_HOME/logs/worker-clean.log`); an exclusive lock
   (`logs/worker-clean.lock`) makes any concurrent pass a no-op; orch.sh
   runs it on `.orch/worktrees` at the end of ready-pickup and pr-fix when cdamux is on
   PATH.
5. **Disk guard** — `cdamux worker spawn` refuses when the worktree filesystem has less
   than `[worktrees].min_free_gib` free (default 40, `0` = off), naming the clean
   command. orch.sh applies the same check before creating a worktree
   (`ORCH_MIN_FREE_GIB`). Both fail open when `df` cannot be read.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Auto-detect by manifest (`Cargo.toml` → `target`) | Misses nested workspaces (`cdamux/target`) and monorepo `node_modules`; the git-ignored filter already makes a name list safe |
| Shared `CARGO_TARGET_DIR` / sccache | Out of scope; Rust only; separate change |
| Remove finished worktrees entirely | Loses inspectable state; a human decision (orch-cleanup.sh stays manual) |
| Hook into `cdamux state gc` | Not run automatically; `gh` calls would slow a pure state pass |

## Consequences

- **Positive**: merged work stops holding gigabytes; spawn fails early with a fix
  instead of a full disk mid-build.
- **Negative**: one `gh` call per finished worktree with build dirs; a worktree reopened
  after its PR merged rebuilds from scratch.
- **Constraint**: never delete outside the worktree; never touch an open PR or a live
  unfinished session.

## References

- `cdamux/crates/cdamux-core/src/worktree_clean.rs`, `cdamux worker clean`
- `skills/core/cda-dev/scripts/lib/worktree.sh` (`orch_disk_guard`, `orch_free_build_dirs`)
- [[ADR-011-cdamux-unified-state-model|ADR-011]], [[ADR-015-global-and-nested-orchestrators|ADR-015]]
