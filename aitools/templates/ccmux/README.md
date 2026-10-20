# ccmux templates (experimental)

Upstream: <https://ccmux.dev/>

1. Install ccmux (`brew install ccmux` or Nix when packaged).
2. Run `ccmux setup` for agent hooks.
3. Prefer kit orch worktrees (`.orch/worktrees/`) for Kanban/PR work — ccmux
   worktree helpers are interactive comfort only (ADR-003).

Copy any local preferences into `$CDA_AITOOLS_HOME/ccmux/` when you outgrow
upstream defaults.
