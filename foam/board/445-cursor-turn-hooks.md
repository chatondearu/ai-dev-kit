---
title: "feat(cdamux): wire Cursor pre-execution hooks so turn is reliable"
status: in-review
priority: P1
updated: 2026-11-25
github_issue: 445
---

## Goal

Wire Cursor pre-execution and lifecycle hooks so `turn` is `running` on prompt and permission-like events, `ended` on a turn-ending `stop`, `exited` on `sessionEnd`.

## Acceptance

- [x] `beforeSubmitPrompt` → `turn=running`
- [x] Turn-ending `stop` → `turn=ended`
- [x] Followup `stop` keeps `turn=running`
- [x] `bash cdamux/hooks/test.sh` green

PR: https://github.com/chatondearu/ai-dev-kit/pull/446
