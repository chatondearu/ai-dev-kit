# PRD-195 — cdamux session resume: restore registry, agent session ids, restore and restart

- **Status**: approved
- **GitHub**: [#195](https://github.com/chatondearu/ai-dev-kit/issues/195)
- **Date**: 2026-11-05
- **Tags**: #cdamux #sessions #resilience

## Problem

cdamux spawns agent sessions on tmux and keeps no record outside tmux
([[../foam/decisions/ADR-004-cdamux|ADR-004]]: "tmux is the database"). When a tmux
window is killed by mistake, when the machine reboots, or when the operator wants to
restart every agent so it reloads an updated kit (skills, hooks, config), the session
is gone and the agent conversation has to be found and resumed by hand — or is lost.

## Goals

- [ ] Every session cdamux spawns is recorded in a restore registry that survives the tmux server
- [ ] The registry holds the agent session id and keeps it current across `/clear` and resumes
- [ ] `cdamux restore <name>|--all` brings dead sessions back on the same conversation
- [ ] Fleet shows dead recorded sessions as dormant rows with a restore key
- [ ] `cdamux restart --all` restarts every recorded agent session on its conversation (kit update)

## Non-goals

- Auto-restore when the tmux server starts
- pi (no resume flag wired yet), orch worker sessions (`ORCH_TMUX`), web UI
- Restoring scrollback or pane contents (tmux-resurrect territory)

## Decisions taken at intake (operator, 2026-11-05)

1. One JSON file per session under `${XDG_STATE_HOME:-~/.local/state}/cdamux/sessions/`; ADR-008 amends ADR-004 (tmux stays the truth for what runs; the registry is a restore snapshot only).
2. Claude gets `--session-id <uuid>` at spawn (works without hooks); `SessionStart` hooks for Claude / Cursor / opencode keep the id current, opt-in via `install.sh --cdamux-hooks`.
3. Restore is explicit: CLI + Fleet dormant rows.
4. `cdamux restart --all` = kill + resume; busy sessions skipped unless `--force`.
5. Closing a session from cdamux forgets it; an external kill or a reboot leaves it dormant.
6. Orch worker sessions are excluded.

## Requirements

See [[../plans/PLAN-195-cdamux-session-resume|PLAN-195]] — one child issue per task
(#196–#199) with scope, acceptance and verify command.

## Success metrics

- On a throwaway tmux server: spawn a Claude session, `kill-server`, `cdamux restore --all` → the session is back and runs `claude --resume <same id>`
- After `/clear` with hooks installed, the registry holds the new id
- `nix develop -c bash scripts/verify.sh` green on `main`

## Links

- Plan: [[../plans/PLAN-195-cdamux-session-resume|PLAN-195]]
- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]], [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]], ADR-008 (written in R1)
- Related: [[PRD-155-cdamux-agent-map-ux|PRD-155]]
