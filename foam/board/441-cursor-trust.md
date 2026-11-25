---
title: "fix(cdamux): pass cursor-agent --trust so workers skip workspace-trust prompt"
status: in-progress
priority: P1
updated: 2026-11-25
github_issue: 441
---

## Goal

Cursor workers start in a new git worktree without a "Trust this workspace" prompt. Pass `--trust` at spawn, restore, and orch.sh `-p`. Never `--force` (ADR-019).

## Acceptance

- [ ] `--trust` is on the argv for Cursor worker spawn and restore
- [ ] orch.sh Cursor `-p` line includes `--trust`
- [ ] `grep -c 'dangerously\|--force'` on `agents.sh` stays 0
- [ ] cdamux worker/restore tests and `test_agents.sh` pass
- [ ] Repo verify passes
- [ ] PR from a worktree with `Closes #441`
