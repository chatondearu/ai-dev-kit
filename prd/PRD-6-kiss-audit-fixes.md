# PRD-6 — Fix kiss-audit findings

- **Status**: approved
- **GitHub**: [#6](https://github.com/chatondearu/ai-dev-kit/issues/6)
- **Date**: 2026-10-13
- **Tags**: #quality #tooling

## Problem

The 2026-10-13 `/kiss-audit` of this kit found 9 defects with concrete repros, and dogfooding the batch found a 10th: global Claude rules are never loaded on a `CLAUDE_CONFIG_DIR` machine, the `/kiss` toggle fires on `/kiss-audit`, "stop kiss" does not persist, the injected repo map shows only the first folder, the install code that rewrites user config has no committed test, the README duplicates and drifts from the skills index, `install.sh -h` truncates, invalid JSON is skipped silently, the Nix path silently lacks kiss hooks, and `new-prd.sh` does not even parse.

## Goals

- [ ] Every finding's repro no longer reproduces
- [ ] Each fix is the smallest change that works; risky config-writing code gets one guarding test

## Non-goals

- Wiring kiss hooks into Nix (document the gap only)
- New features; `archive/`; a repo-map redesign beyond a dir-level listing
- `new-plan.sh` child-plan support (tracked separately in #10)

## Requirements

### Functional

- [ ] #7 — install: CLAUDE.md into `$CLAUDE_HOME`, full `-h`, warn on invalid JSON, committed hook-wiring test
- [ ] #8 — kiss hooks: exact `/kiss` match, sticky off per project, dir-level repo map
- [ ] #9 — README: link `skills/README.md` as the only index, document the Nix hooks gap
- [ ] #11 — `new-prd.sh` parses and runs; `bash -n` guard over kit scripts

### Non-functional

- [ ] No test or install step touches the real user config (temp HOME / config dirs only)
- [ ] Existing cda-dev suite + kiss lib test stay green

## Related project memory

- ADRs: none yet (first foam bootstrap of this repo)

## Success metrics

- 4 child PRs CI-green and mergeable; audit repros re-run clean

## Implementation pointer

`plans/PLAN-7-install-fixes.md`, `plans/PLAN-8-kiss-hooks-fixes.md`, `plans/PLAN-9-readme-fixes.md`, `plans/PLAN-11-new-prd-syntax.md`
