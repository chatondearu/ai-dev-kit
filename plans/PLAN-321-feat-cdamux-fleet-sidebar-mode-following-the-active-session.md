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

## Spike decision

**One sidebar pane per tmux client, moved with `join-pane`** (not one per window): a
per-window sidebar needs a live process and cursor sync per window, while moving the
single pane keeps one list. tmux facts the spike pinned down (tmux 3.6):

- `display-message -c <client>` names the client but resolves `#{pane_id}` from the most
  recently used session, wrong with two clients; resolve the client's session from
  `list-clients` and target `=<session>:` instead.
- Every view change uses `switch-client -c <client>`, so the other client's session never changes.
- `join-pane -h -f [-b] -l N` keeps a full-height column; `-d` leaves the main pane active
  (used when the client moved on its own); `select-pane -l` returns to the pane the sidebar
  sits next to (Enter).
- Two clients: the sidebar follows the invoking client only; a second client viewing the window
  the sidebar leaves loses the pane but keeps its session and panes.
- The sidebar also follows a client that switches session by other means (prefix-s, M-u),
  polled every 500 ms, without taking focus.

## Tasks

- [x] Spike + decision recorded
- [x] Narrow Fleet layout + snapshot
- [x] Switch on cursor move + follow + tests on `-L` server
- [x] Enter / close + doc

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Moving panes steals another client's window | Always pass `-c <client>`; test with two clients |
| Rapid cursor moves thrash | Debounce switches (~150 ms) |

## Test plan

- [x] Throwaway-tmux test: sidebar follows across 3 sessions; second client unaffected

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
