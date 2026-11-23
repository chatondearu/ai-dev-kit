---
title: "fix(cda-kiss): Cursor beforeSubmitPrompt ignores additional_context"
status: backlog
priority: P2
updated: 2026-11-23
github_issue: 237
---

## Goal

The `/kiss` level switch reaches the agent on Cursor, not only on Claude Code.

## Acceptance

- [ ] Live `cursor-agent` run confirms whether `additional_context` on `beforeSubmitPrompt` is read
- [ ] If not: move the text to a supported channel (`sessionStart`, `user_message`) or drop it on Cursor

State 2026-11-23: `hooks/lib.js` still sends `additional_context` on `beforeSubmitPrompt`; unverified live.
