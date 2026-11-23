---
title: "fix(cdamux): one CDAMUX_HOME regardless of the env that started tmux"
status: backlog
priority: P1
updated: 2026-11-23
github_issue: 391
---

## Goal

Every cdamux process and hook resolves the same state home, so a new orchestrator sees workers started from another env.

## Acceptance

- [ ] Product decision recorded: config key, Nix-exported env, or doctor warning (options in the issue)
- [ ] `cdamux_home()` and `hooks/ledger.js` agree on the chosen source
- [ ] Verify green

Needs an owner decision before Ready.
