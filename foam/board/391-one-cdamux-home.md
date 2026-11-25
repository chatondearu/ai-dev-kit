---
title: "fix(cdamux): one CDAMUX_HOME regardless of the env that started tmux"
status: in-review
priority: P1
updated: 2026-11-25
github_issue: 391
---

## Goal

Every cdamux process and hook resolves the same state home, so a new orchestrator sees workers started from another env.

## Acceptance

- [x] Product decision recorded: one exported `CDAMUX_HOME`, propagated into every spawned pane (Q8, ADR-011 amendment 2026-11-23)
- [x] `cdamux_home()` and `hooks/ledger.js` agree on the chosen source (panes inherit it)
- [x] `cda.aitools.cdamux.home` exports `CDAMUX_HOME` (shells + systemd/launchd)
- [ ] Owner runs `cdamux state migrate-home --apply` once on the maintainer host
- [ ] Verify green
