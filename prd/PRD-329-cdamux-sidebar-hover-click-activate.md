# PRD-329 — cdamux sidebar hover and click-to-activate

- **Status**: approved
- **GitHub**: [#329](https://github.com/chatondearu/ai-dev-kit/issues/329)
- **Date**: 2026-11-14
- **Tags**: #product #cdamux

## Problem

`cdamux sidebar` (#321) is keyboard-only and disables mouse capture so tmux
owns clicks. Users who point at a session get no hover feedback, and a click
does not activate (follow + focus) the way Enter does.

## Goals

- [ ] Distinct hover style on sidebar tree rows (not the same as selection)
- [ ] Left-click on a live session activates it (same as Enter)
- [ ] Project rows keep fold/select; dormant keeps current Enter message
- [ ] Keyboard j/k debounce switch unchanged; popup Fleet untouched

## Non-goals

- Popup Fleet mouse changes
- Hover-to-switch (do not switch on hover)
- Dormant restore from the sidebar
- Reopening or redesigning #321 beyond mouse UX

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer in tmux | Opens `cdamux sidebar`, moves the mouse over sessions, sees hover, clicks to land focus on that session's main pane |

## Requirements

### Functional

- [ ] Enable mouse capture in the sidebar `--in-pane` loop
- [ ] Track hover row on mouse move; draw a style distinct from selection
- [ ] Left-click live session → existing Attach path (`follow_session` + `focus_main_pane`)
- [ ] Left-click project → fold/select only; dormant → current Enter message
- [ ] Footer mentions click / hover

### Non-functional

- [ ] Two-client contract from PLAN-321 unchanged (`-c <client>`)
- [ ] Verify green for the shipping PR

## Related project memory

- ADRs: [[../foam/decisions/ADR-004-cdamux|ADR-004]], [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]]
- Prior: [[../plans/PLAN-321-feat-cdamux-fleet-sidebar-mode-following-the-active-session|PLAN-321]], epic [#315](https://github.com/chatondearu/ai-dev-kit/issues/315)

## Open questions

_(none — settled in intake)_

## Success metrics

- Click activates; hover is visible; j/k still switches; no popup regression

## Implementation pointer

Child plan: `plans/PLAN-331-feat-cdamux-sidebar-hover-and-click-to-activate.md`
Child issue: [#331](https://github.com/chatondearu/ai-dev-kit/issues/331)
