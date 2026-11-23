---
title: "epic: cdamux session resume (restore registry, session ids, restore/restart)"
status: done
priority: P1
updated: 2026-11-23
github_issue: 195
---

## Goal

Restore a cdamux session on the same agent conversation after a window kill, a reboot, or a restart.

## Acceptance

- [x] #196 restore registry + ADR
- [x] #197 SessionStart id hooks
- [x] #198 `cdamux restore`
- [x] #199 Fleet dormant rows + `restart --all`
- [x] Follow-up #415 (PR #416): prune unrestorable + `restore --last-orch`

Closed 2026-11-23.
