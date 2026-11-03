# PLAN-129 — Fix kiss-audit 2026-11-03 with a fleet of subagents

- **Status**: in-progress
- **PRD**: [[../prd/PRD-129-kiss-audit-2026-11-03|PRD-129]]
- **GitHub**: [#129](https://github.com/chatondearu/ai-dev-kit/issues/129)
- **Date**: 2026-11-03

## Summary

Close the 2026-11-03 audit with 11 Task subagents in 3 lanes (H harness, S
scripts, C cdamux), one worktree + branch + PR each. Same procedure as
[[../foam/archive/plans/PLAN-90-kiss-audit-2026-11|PLAN-90]]: `orch.sh` is not
used because its gates and lock are among the bugs.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | orch-auto opt-in; no auto-merge |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux is the source of truth; tailnet-only, never `0.0.0.0` |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | workers → integrator → verifier |
| [[../foam/decisions/ADR-006-review-gate-fail-closed-verify|ADR-006]] | reviewer + verifier fail closed — H1 amends it (markers cleared per pass, both loop modes) |

## Tasks

The full contract of each task (finding, repro, fix, verify) is the child issue body.

| Id | Child | Task | Model | Depends |
| -- | ----- | ---- | ----- | ------- |
| H1 | #130 | Harness gates actually gate (verifier markers, exports, no-checks, `--no-graph`, reviewer PRD_PATH) | opus | — |
| H2 | #136 | Untrusted GitHub input (brief author, fork pr-fix → blocked, orch-auto fail-closed) | opus | H1 |
| H3 | #140 | mkdir lock stale detection, safe worktree reuse, `{{BRANCH}}`, atomic status.json, frontier parser, dead code | sonnet | H2 |
| S1 | #131 | `--init-foam --force` never deletes | sonnet | — |
| S2 | #132 | Green verify on darwin (devShell, realpath test, missing suites, moshi-hook-pair) | sonnet | — |
| S3 | #137 | gh-board: Closes/Fixes only, review events, cursor on error, pagination | sonnet | S2 |
| S4 | #133 | install.sh bash 3.2, prompt hooks, uninstall; seed-design; doctor | sonnet | — |
| S5 | #134 | doc-hygiene/new-prd/new-plan/import-kanban; delete foam-graph-export | haiku | — |
| C1 | #135 | cdamux exact pane targets `=name:`, bracketed paste | sonnet | — |
| C2 | #138 | Tailnet body limit before auth, JSON-only POST, Host check, exit on failure | opus | C1 |
| C3 | #139 | TUI selection by name, composer keys, mouse offset, spawn order, dead code | sonnet | C1 |

Wave 1: H1, S1, S2, S4, S5, C1. Wave 2 (after its dependency merges): H2, S3, C2, C3. Wave 3: H3.

- [ ] H1 · [ ] H2 · [ ] H3
- [ ] S1 · [ ] S2 · [ ] S3 · [ ] S4 · [ ] S5
- [ ] C1 · [ ] C2 · [ ] C3

## Execution

Worktree `.orch/worktrees/<Id>-issue-<n>`, branch `fix/<n>-<slug>` from
`origin/main`. Prompt = PLAN-90 subagent template, with the child issue body as
the contract. cdamux tasks follow `cdamux/AGENTS.md` § Testing against tmux.

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Same file in one lane (`orch.sh`, `tui.rs`) | Depends ordering; next task starts from `main` after the merge |
| A worker kills the live tmux server (happened in PLAN-90) | Rule restated in every C prompt |
| `install.sh` run against the real config | Temp `HOME`/`CLAUDE_CONFIG_DIR`/`CURSOR_HOME`/`XDG_CONFIG_HOME` in every prompt |

## Verification

- [ ] `nix develop -c bash scripts/verify.sh` green on `main`
- [ ] PRD-129 → shipped, this plan → done and archived, `foam/index.md` updated
