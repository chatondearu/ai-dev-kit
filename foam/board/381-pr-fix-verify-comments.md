---
title: "fix(cda-dev): pr-fix accepts AGENTS.md verify block with comment lines"
status: backlog
priority: P1
updated: 2026-11-23
github_issue: 381
---

## Goal

`orch.sh --mode pr-fix` resolves this repo's verify block to `bash scripts/verify.sh` and runs it in untrusted mode.

## Acceptance

- [ ] Rule 0 drops comment lines from the extracted block
- [ ] Untrusted `orch_run_verify` accepts this repo's verify; a non-allowlisted real command is still refused
- [ ] Test in `skills/core/cda-dev/scripts/tests/`
- [ ] Verify green
