---
title: "fix(cda-kiss): Cursor beforeSubmitPrompt ignores additional_context"
status: done
priority: P2
updated: 2026-11-23
github_issue: 237
---

## Goal

The `/kiss` level switch reaches the agent on Cursor, not only on Claude Code.

## Acceptance

- [x] Evidence that `additional_context` on `beforeSubmitPrompt` is read: ponytail
  `docs/cursor-hooks.md` (upstream `67a1601`) — the Cursor 3.20.17 client's response
  validator accepts it, the submit path injects it as a system reminder (inline up to
  10,000 chars), and a live level-switch check passed. Undocumented, so version-dependent.
- [x] Kit already emits the same shape (`{ additional_context, continue: true }` in
  `hooks/lib.js` `writeHookOutput`): no code change; caveat added to `cda-kiss` SKILL.md.

Not done: a live `cursor-agent` run on this kit's hooks (re-check if a Cursor update drops the context).
