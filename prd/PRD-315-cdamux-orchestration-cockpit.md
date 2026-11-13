# PRD-315 — cdamux orchestration cockpit

- **Status**: approved
- **GitHub**: [#315](https://github.com/chatondearu/ai-dev-kit/issues/315)
- **Date**: 2026-11-14
- **Tags**: #product #cdamux #orchestration

## Problem

The maintainer wants to work from **one session**, Orca-like: follow workers,
receive their reports and answer their asks without leaving it. PRD-239 shipped
the model and recent fixes (#305, #310–#314) closed the gaps an audit found, but
nothing proves the loop end to end. Around it, three daily frictions remain:

- the Foam tab is a flat list with a rendered preview only: no tree, no click,
  no way to follow a `[[wikilink]]` or read the raw source;
- a session does not know its PR or source issue, so reviewing means searching
  GitHub by hand;
- Fleet is a popup: it hides the session view and must be reopened to see
  statuses change.

## Goals

- [ ] The orchestration loop is proven by an automated scenario and a short
      live checklist (#316)
- [ ] Foam tab: collapsible tree, keyboard + mouse, wikilinks, raw/rendered
      toggle (#317, #318)
- [ ] Sessions linked to PR / issue, visible and clickable in Review and Fleet
      (#319, #320)
- [ ] Fleet sidebar mode next to the session view (#321)

## Non-goals

- Editing foam from cdamux; a wikilink graph view
- Browsing `prd/` / `plans/` / `doc/` (ADR-012: `foam/` only)
- Installing tmux key binds (ADR-001)
- GitHub write operations from cdamux
- Signal hygiene / nudge policy (#273, #278)

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Runs the dogfood checklist: orchestrator spawns two workers, one asks, the maintainer answers from the orchestrator, both report `done` with a PR |
| Maintainer | Opens the Foam tab, expands `decisions/`, clicks ADR-011, follows `[[ADR-004-cdamux]]`, presses `m` to read the raw source |
| Maintainer | Selects a worker in Review: header shows `PR #312 open · issue #273`; clicks it or presses `o` |
| Maintainer | Runs `cdamux sidebar --left`: Fleet stays on the left; moving down the list switches the right side to each session; statuses update live |

## Requirements

### Functional

- [ ] #316: e2e scenario on a throwaway tmux (`TMUX_TMPDIR` + `-L`), temp
      `CDAMUX_HOME`, stub clients: `orch new` → `worker spawn` (spec) → `ask` →
      `answer` → `done` → digest/nudge; asserts statuses, badges, delivery
      outcome, spec intact. Live checklist in `cdamux/docs/dogfood.md`. Gaps →
      follow-up issues.
- [ ] #317: tree of `foam/` with collapsible folders; keys + mouse; wikilinks
      `[[name]]` / `[[path|label]]` resolved under `foam/`, Enter opens, back
      returns. ADR-012 amended for wikilinks.
- [ ] #318: `m` toggles raw / rendered; state kept while browsing.
- [ ] #319: PR from the session branch (`gh pr list --head`, cached with a
      TTL in agent state), issue from `Closes #N` or the branch number;
      `orch.sh` workers record theirs; `cdamux link` override wins.
- [ ] #320: Review header + Fleet badge; OSC 8 hyperlinks; `o` opens in the
      browser.
- [ ] #321: `cdamux sidebar [--left|--right] [--width N]`; one sidebar follows
      the user; cursor move switches session, Enter focuses; popup unchanged;
      safe with two clients attached.

### Non-functional

- [ ] No `gh` call on the refresh path (cache, background or on demand)
- [ ] Tests on throwaway tmux servers only (`cdamux/AGENTS.md`)
- [ ] Without `gh`, offline, or without OSC 8 support: features degrade
      silently (no link, plain text)
- [ ] Verify green per PR

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]],
  [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]],
  [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]],
  [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]]
- PRDs: [[PRD-239-orchestrator-session|PRD-239]],
  [[PRD-265-cdamux-foam-browser|PRD-265]]

## Open questions

1. Sidebar mechanics (#321): move one pane with `join-pane` on each switch, or
   one sidebar per window? Decide in the plan after a spike on two clients.
2. OSC 8 through tmux needs `terminal-features ',*:hyperlinks'`: document only.

## Success metrics

- #316 scenario passes in `cargo test`; the live checklist runs in ≤ 15 min
- In the sidebar, switching across 5 sessions with arrow keys needs no other key
- A worker created by `orch.sh` shows its PR in Review with no manual step

## Implementation pointer

One plan per child: `plans/PLAN-316-*.md` … `plans/PLAN-321-*.md`.
