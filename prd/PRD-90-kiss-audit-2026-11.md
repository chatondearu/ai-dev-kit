# PRD-90 — Fix kiss-audit 2026-11-01 (harness gate, skills routing, cdamux)

- **Status**: approved
- **GitHub**: [#90](https://github.com/chatondearu/ai-dev-kit/issues/90)
- **Date**: 2026-11-01
- **Tags**: #quality #tooling #architecture

## Problem

The 2026-11-01 `/cda-kiss-audit` of the whole kit (bash harness, skills/prompts,
cdamux) found 20 defects with concrete repros. The two that defeat the kit's
purpose ("the agent understands, asks, splits, launches, delivers — I verify and
decide"): **no independent agent reads the diff before a PR** (`reviewer.md` is
never rendered, verifier off by default) and **`verify` is trivially green**
(`true` fallback, the `verify:` line of `AGENTS.md` shadows the real test block,
retries never see the failing output). A plain chat request has no route into
intake → PRD → plan → orch, and worker prompts lose PRD/ADR context (`N/A`).
In cdamux, `send` types the literal word `Enter`, session names with `.` break
every `-t` target, `has-session` prefix-matches, and the tailnet API accepts
`0.0.0.0` with no auth.

## Goals

- [ ] Findings 1–20 of the audit no longer reproduce (each fix carries one test
      when the logic is non-trivial)
- [ ] A Ready issue picked by `orch.sh` gets an independent review and a real
      verify before `done`; "no verify command" blocks instead of passing
- [ ] The fixes are delivered by parallel subagents with a model matched to task
      difficulty; the operator only reviews PRs and decides

## Non-goals

- Auto-merge
- New orch features beyond these fixes (no new modes, no new config surface
  beyond `orch: allow-no-verify`)
- Rewriting the cdamux TUI; only the hot loop and `run_dashboard` split
- Deleting personal / productivity skills

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Senior product engineer (operator) | Asks for a feature in chat; the agent routes to intake, asks one frontier of questions, splits work, launches workers; PRs arrive reviewed and verified |
| AFK `orch-auto` timer | Picks Ready cards; a lazy worker is caught by reviewer/verify, not by the operator |
| cdamux user | `cdamux send … --enter` submits; projects named `web.app` spawn and label correctly; tailnet refuses `0.0.0.0` |

## Requirements

### Functional

1. `orch_ready_pickup_one` dispatches `reviewer.md` in a fresh agent context after
   the worker loop; `changes_requested` → one fix iteration, then `blocked`.
2. `orch_resolve_verify` prefers a fenced "Verify" block over a `verify:` line;
   no command → `blocked` unless the issue body contains `orch: allow-no-verify`.
3. The retry prompt includes the tail of the failed verify output.
4. `verifier.md` fails on added `TODO|FIXME|\.skip\(|xit\(` or removed tests not
   justified by the plan.
5. `{{PRD_PATH}}`, `{{EPIC}}`, `{{CONSTRAINTS}}` are resolved from `prd/` and
   `foam/decisions/`, never `N/A` when the files exist.
6. `AGENTS.base.md` routes any feature/fix request (1 or N subjects) to `cda-dev`
   intake; intake delegates questioning to `cda-grill` (one frontier round).
7. cdamux `send_to_session` delegates to `tmux::send_line_to_session`;
   `session_name_for_path` replaces `.`/`:`; `has-session` uses `=name`;
   `apply_pane_split` targets the session.
8. Tailnet refuses unspecified bind IPs, canonicalizes `?repo=` paths, and checks
   a shared secret header.

### Non-functional

- Each fix is the smallest change that works (KISS full).
- Fleet refresh issues at most `2 + N` tmux processes per tick for N sessions.
- No worker runs `install.sh` against the real user config.

## Success metrics

- `for t in …; do bash "$t"; done` (AGENTS.md Verify block) is green on `main`.
- `cargo test` green, 0 dead-code warnings.
- A dry `orch.sh --mode ready-pickup` on a repo without tests ends `blocked`,
  not `done`.

## Open questions

- Shared-secret header for tailnet: env var name and rotation — decide in the
  task, record in ADR-004 amendment or `cdamux/docs/tailnet-api.md`.

## Links

- Plan: [[../plans/PLAN-90-kiss-audit-2026-11|PLAN-90]]
- Prior batches: [[PRD-6-kiss-audit-fixes|PRD-6]], [[PRD-25-kiss-audit-orch-gh-board|PRD-25]]
- ADRs: [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[../foam/decisions/ADR-004-cdamux|ADR-004]],
  [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]]
