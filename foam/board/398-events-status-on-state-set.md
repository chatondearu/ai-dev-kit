---
title: "fix(cdamux): finalize/dismiss state_set paths emit events-feed status items"
status: in-review
priority: P2
updated: 2026-11-23
github_issue: 398
---

## Goal

Status changes written with `state_set` (finalize, dismiss, sweep-orphans) emit a `status` item in the events feed, like `state_append`.

## Acceptance

- [x] Killing a worker host via finalize produces a `status` item `error` with the right `prev`
- [x] `cdamux/docs/events.md` no longer lists the `state set` gap
- [ ] Verify green
