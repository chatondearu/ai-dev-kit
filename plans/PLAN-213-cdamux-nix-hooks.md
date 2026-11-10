# PLAN-213 — Nix cdamux package wires and updates agent hooks

- **Status**: done
- **GitHub**: [#213](https://github.com/chatondearu/ai-dev-kit/issues/213)
- **Related**: [[../prd/PRD-195-cdamux-session-resume|PRD-195]]
- **Date**: 2026-11-10

## Summary

The cdamux hooks (sub-agent ledger, `SessionStart` id capture) ship in the Nix
package with one `cdamux-hooks wire|unwire` script. An opt-in `cda.aitools`
option runs it on every activation, so a rebuild that updates cdamux also updates
the hooks every agent CLI runs. `install.sh --cdamux-hooks` delegates to the same script.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | opt-in; plain `install.sh` never wires cdamux hooks |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | `cda.aitools` is experimental and opt-in; never enabled by `install.sh` |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | hooks fail-open, local only; strip only cdamux entries |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | `SessionStart` keeps the restore-registry id current |

## Tasks

- [x] Package: install `cdamux/hooks/*.js` to `$out/share/cdamux/hooks`, a `node` link to the Nix nodejs, `bin/cdamux-hooks`
- [x] `cdamux-hooks wire|unwire [claude] [cursor] [opencode]` (config dirs from `CLAUDE_CONFIG_DIR` / `CURSOR_HOME` / `XDG_CONFIG_HOME`): stable symlink `${XDG_DATA_HOME:-~/.local/share}/cdamux/hooks`, strip any `/cdamux/hooks/` entry, wire Claude / Cursor / opencode; idempotent
- [x] `install.sh --cdamux-hooks` / `--uninstall` delegate to the script (checkout hooks dir)
- [x] `cda.aitools.cdamux.hooks.enable` (default false) + `claudeConfigDir`; darwin launchd `RunAtLoad` oneshot, NixOS systemd user oneshot; log hint `cdamux restart --all`
- [x] Tests on temp `HOME` / `CLAUDE_CONFIG_DIR` / `CURSOR_HOME` / `XDG_*`: idempotent, replaces checkout entries, keeps kiss + foreign, unwire, opencode link
- [x] Docs: `aitools/README.md`, `cdamux/docs/subagents.md`
- [x] Verify: `nix develop -c bash scripts/verify.sh`, `nix build .#cdamux`, module eval with the option on

## Risks

- Never run against the live config (`/cda/.claude`, `~/.cursor`): tests override every config dir.
- A store path in `settings.json` would churn per rebuild and defeat `strip()`: only the stable symlink path is written.
