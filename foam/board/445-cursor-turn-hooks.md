---
title: "feat(cdamux): wire Cursor pre-execution hooks so turn is reliable"
status: in-progress
priority: P1
updated: 2026-11-25
github_issue: 445
---

## Goal

Wire Cursor pre-execution and lifecycle hooks so `turn` is `running` on prompt and permission-like events, `ended` on a turn-ending `stop`, `exited` on `sessionEnd`.

## Acceptance

- [ ] `beforeSubmitPrompt` → `turn=running`
- [ ] Turn-ending `stop` → `turn=ended`
- [ ] Followup `stop` keeps `turn=running`
- [ ] `bash cdamux/hooks/test.sh` green
