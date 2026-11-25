---
title: "fix(cda-orchestrator): allow git fetch and git pull --ff-only on the orch checkout"
status: in-review
priority: P1
updated: 2026-11-25
github_issue: 439
---

## Goal

Allow a cdamux orchestrator to `git fetch` and `git pull --ff-only` on its current checkout so a human "pull then …" is not refused; keep every other repo mutation on workers.

## Acceptance

- [ ] Maintainer saying "pull main" in an orch session is covered by the written exception
- [ ] `bash skills/quality/cda-kiss/hooks/test.sh` passes
- [ ] Repo `bash scripts/verify.sh` (or `mise run verify`) passes
- [ ] PR opened from a worktree (not `--here`, never onto main)
