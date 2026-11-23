---
title: "chore(kit): roll out foam/board to non-Beamy cdamux projects"
status: done
priority: P2
updated: 2026-11-23
github_issue: 408
blocked_by: [405]
---

Kit dogfood lives in this repo's `foam/board/`. Per-repo boards live on those
remotes; tracking is local `$CDAMUX_HOME`, not this file. Skip Beamy by
org/remote (`beamysoftware/*`). Plan: `plans/PLAN-408-foam-board-rollout.md`.

Shipped in PR #414 (kit side; per-repo boards tracked locally).
