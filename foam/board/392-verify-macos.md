---
title: "test(kit): verify.sh green on macOS hosts"
status: backlog
priority: P1
updated: 2026-11-23
github_issue: 392
---

## Goal

`bash scripts/verify.sh` passes on the maintainer's macOS host without `nix develop`.

## Acceptance

- [ ] `smoke_orch_dry.sh` skips the process-group check when `setsid` is missing
- [ ] `cdamux/hooks/test.sh` no longer flaky (timeouts, `CDAMUX_HOME` isolation)
- [ ] `home_repair_tmux.rs` accepts `/bin/sh` being bash
- [ ] Verify green on macOS and Linux
