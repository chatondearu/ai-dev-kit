---
title: "fix(cdamux): wake live Cursor after kind=done instead of inbox"
status: in-progress
priority: P1
updated: 2026-11-25
github_issue: 444
---

## Goal

After a worker sends `kind=done`, a live pane must still receive `relay`/`answer` via the ADR-018 wake line (`📨`), not `inbox`. Never paste a message body (`cdamux send`).

## Acceptance

- [ ] Cursor (or fake) agent `status=done`, `turn=ended`, live pane, quiet → `relay` is `scheduled` or `woken`, not `inbox`
- [ ] `needs_input` still `inbox` / After
- [ ] ADR-018 amendment; `wake_plan` + `delivery_tmux` tests; architecture + orchestrator skill line
