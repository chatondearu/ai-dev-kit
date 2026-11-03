# PLAN-177 — cdamux home session + no kick-out on session close

- **Status**: verify
- **PRD**: none (fix; issue body is the contract)
- **GitHub**: [#177](https://github.com/chatondearu/ai-dev-kit/issues/177)
- **Date**: 2026-11-03

## Summary

Outside tmux, dashboard-mode `cdamux` enters a dedicated home session named
`cdamux` (`tmux new-session -A -s cdamux cdamux`) so the server starts and
continuum can restore. Hide that session from Fleet / Sessions. Set
`detach-on-destroy next` so closing a session never kicks the client out of
tmux; ensure the home session exists before killing the last real session the
client is attached to.

## Continuum probe (worker finding)

On a throwaway `tmux -L` server (tmux 3.6a): `tmux start-server` alone does
**not** create sessions and does **not** fire a usable restore path for an
empty server. Continuum restore still needs a config that loads the plugin
**and** a server start that loads that config — creating/attaching the home
session is the right trigger. Do **not** replace the home session with bare
`start-server`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux\|ADR-004]] | tmux is the database; cdamux stays in-tree |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy\|ADR-007]] | throwaway tmux only in tests; plain `install.sh` never wires cdamux hooks |
| [[../foam/decisions/ADR-001-host-tools-vs-kit\|ADR-001]] | kit templates only; maintainer `/etc/tmux.conf` is out of scope (PR notes the line) |
| [[../foam/decisions/ADR-003-cda-aitools\|ADR-003]] | aitools templates are the shared tmux source |

## Scope

### In scope

- Outside-tmux dashboard → home session `cdamux`
- Hide `cdamux` from Fleet / Sessions / list consumers of `list_tmux_sessions`
- `set -g detach-on-destroy next` in `aitools/templates/tmux.conf`
- Before `x` kills the attached last real session: ensure home exists
- Unit test(s) for filter / ensure-before-kill / outside-tmux argv
- Docs: `cdamux/docs/architecture.md` (+ template note)

### Out of scope

- resurrect / continuum plugin changes
- Fleet sort order / cdamux-driven next switching
- Maintainer host `/etc/tmux.conf` edit

## Tasks

- [x] Constant + helpers: home session name, filter, ensure, current session, ensure-before-kill predicate
- [x] `list_tmux_sessions` omits the home session
- [x] Dashboard outside `$TMUX`: `tmux new-session -A -s cdamux cdamux` (exec); `--pick` / subcommands unchanged
- [x] Kill path ensures home when needed; template `detach-on-destroy next`
- [x] Unit tests for pure logic
- [x] Docs + plan continuum finding recorded

## Test plan

- [x] Seams: home filter on parsed list; `should_ensure_home_before_kill`; outside-tmux argv shape
- [x] Manual (throwaway `TMUX_TMPDIR` + default socket under temp): home hidden from `cdamux list`; home remains after killing real sessions

## Verification

- [ ] `nix develop -c bash scripts/verify.sh` (list unrelated failures in PR if any)
- [x] PR notes: copy `set -g detach-on-destroy next` into host tmux.conf if not on aitools module
