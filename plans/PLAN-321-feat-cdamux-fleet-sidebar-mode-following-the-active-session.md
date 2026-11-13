# PLAN-321 — feat(cdamux): Fleet sidebar mode following the active session

- **Status**: ready
- **PRD**: [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]]
- **GitHub**: [#321](https://github.com/chatondearu/ai-dev-kit/issues/321)
- **Date**: 2026-11-14

## Summary

`cdamux sidebar [--left|--right] [--width N]` keeps a live Fleet pane next to the session view; cursor move switches session, Enter focuses; one sidebar follows the user; popup unchanged.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | No bind installed; document an example |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Session model unchanged |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Spike (first task): `join-pane` move of one sidebar pane on each switch vs one sidebar per window; decide on two attached clients; record the choice in this plan
- Sidebar layout of Fleet (narrow rows, statuses, badges)
- Cursor move → `switch-client -c <this client> -t <session>`, sidebar re-attached next to the target window's active pane
- Enter → `select-pane` on the session pane; `q` closes the sidebar
- Doc bind example

### Out of scope

- Changing the popup flow
- Multi-window layouts beyond the active window

## Technical approach

`main.rs` subcommand `sidebar`; reuse Fleet tree/draw with a narrow layout; tmux calls through `crates/cdamux-core/src/tmux.rs`; target the invoking client (`#{client_name}`) to avoid moving another client's view.

## Tasks

- [ ] Spike + decision recorded
- [ ] Narrow Fleet layout + snapshot
- [ ] Switch on cursor move + follow + tests on `-L` server
- [ ] Enter / close + doc

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Moving panes steals another client's window | Always pass `-c <client>`; test with two clients |
| Rapid cursor moves thrash | Debounce switches (~150 ms) |

## Test plan

- [ ] Throwaway-tmux test: sidebar follows across 3 sessions; second client unaffected

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
