---
title: "fix(cdamux): orchestrator nudge uses the singular for one message"
status: backlog
priority: P2
updated: 2026-11-23
github_issue: 322
---

## Goal

`nudge_text(1)` uses the singular.

## Acceptance

- [ ] `nudge_text(1)` reads `📨 1 message — …`; n > 1 unchanged
- [ ] `msg.rs` unit test + `orch_loop_tmux.rs` hop updated
- [ ] Verify green
