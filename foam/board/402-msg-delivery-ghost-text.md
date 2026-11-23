---
title: "fix(cdamux): Claude Code prompt suggestion counts as typed input, msg delivery queued forever"
status: in-progress
priority: P1
updated: 2026-11-23
github_issue: 402
---

## Goal

A pane whose input holds only a Claude Code prompt suggestion (dim ghost text) counts as idle and empty, so queued messages are delivered.

## Acceptance

- [ ] Fixture pane with a dim suggestion → delivery pastes
- [ ] Fixture pane with a real (non-dim) draft → still queued

Worker in progress (2026-11-23).
