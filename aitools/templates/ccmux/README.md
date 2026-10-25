# ccmux templates (experimental)

Upstream: <https://ccmux.dev/>

1. Install ccmux (`brew install ccmux` or Nix when packaged).
2. Run `ccmux setup` for agent hooks.
3. Prefer kit orch worktrees (`.orch/worktrees/`) for Kanban/PR work — ccmux
   worktree helpers are interactive comfort only (ADR-003).

## Prefs (`ccmux.json`)

Copy `ccmux.json` to `~/.config/ccmux/ccmux.json` (or `$CDA_AITOOLS_HOME/ccmux/`
as a mirror). If Claude Code uses a relocatable config dir (e.g.
`CLAUDE_CONFIG_DIR=/cda/.claude`), set:

```json
{
  "additionalClaudeConfigDirs": ["/cda/.claude"]
}
```

## tmux binds

See `../tmux.conf` — `M-p` / `C-p` open the picker. Ghostty needs
`macos-option-as-alt = left` (see `../ghostty/config`) or Option+p inserts « π ».

Session state: `ccmuxd` sets `@ccmux_mark` on windows (shown in the window list).
`ccmux list` and the TUI read the same state from the daemon. Per-session status
bars on `c-*` sessions are applied by ccmux on attach/create, not via `@ccmux_status`
(removed upstream in 0.6.x).
