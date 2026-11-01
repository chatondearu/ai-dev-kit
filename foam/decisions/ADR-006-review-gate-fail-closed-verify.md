# ADR-006 — Independent review gate and fail-closed verify

- **Status**: accepted
- **Date**: 2026-11-01
- **Tags**: #architecture #tooling #quality #autonomy

## Context

The 2026-11-01 kiss-audit ([[../../prd/PRD-90-kiss-audit-2026-11|PRD-90]],
findings 1 and 2) showed the two defects that defeat the kit's purpose ("the
agent delivers, I verify and decide"):

1. **Nobody independent reads the diff.** `prompts/reviewer.md` was never
   rendered by any script; the worker "reviewed" its own work inside the same
   context, and the grounding verifier was off by default (`ORCH_GRAPH_VERIFY=0`).
2. **Verify was trivially green.** `orch_resolve_verify` fell back to `true`
   when nothing matched, a trailing `verify:` line in `AGENTS.md` shadowed the
   real "Verify" block, and a failed verify re-sent the same prompt blind.

[[ADR-002-orch-auto-and-fast-lane|ADR-002]] (AFK pickup) makes this worse: a
lazy worker is caught by nobody before the operator opens the PR.

## Decision

1. **Independent review gate** in `orch_ready_pickup_one` (after the worker
   loop and once the PR exists): render `prompts/reviewer.md` into a fresh
   prompt and run it in a fresh agent call. The reviewer ends with
   `REVIEW: approved` or `REVIEW: changes_requested` (no verdict = changes
   requested). `changes_requested` → exactly **one** worker fix iteration whose
   prompt carries the findings → one re-review → still `changes_requested` →
   status `blocked`, reason `review: changes_requested`. No new mode, no knob.
2. **`ORCH_GRAPH_VERIFY` defaults to `1`**: the grounding verifier is part of
   the gate; `0` disables it explicitly.
3. **Verify resolution prefers the fenced block** under a `Verify` heading or
   bold line in `AGENTS.md` over a `verify:` line; the block runs under `set -e`.
4. **Fail closed**: no verify command found by any heuristic → `blocked` with
   reason `verify: no command (add 'orch: allow-no-verify' to the issue body to
   skip)`. The silent `true` fallback is gone.
5. **Retry prompts carry evidence**: when verify fails, the next iteration's
   prompt appends `## Previous verify output (failed)` with the last 40 lines.
6. **Verifier rules**: `prompts/verifier.md` fails when the diff adds
   `TODO|FIXME|\.skip\(|xit\(` or deletes test files the plan's Test plan does
   not name, quoting the offending hunk.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Keep the worker's self-review (worker.md step 5) as the only review | Same context that wrote the code; finding 1 stays |
| Fail-open verifier/reviewer when the agent prints no verdict | Rubber-stamp by silence; the gate must fail closed |
| Unlimited review ↔ fix rounds | Burns agent calls AFK; two verdicts are enough to hand a human a clear diff |
| Keep `true` fallback with a warning | The warning was already there and ignored; `done` on an unverified diff is the defect |
| A new `ORCH_REVIEW=0` knob | PRD-90 forbids new config surface beyond `orch: allow-no-verify` |
| Per-node review in `--graph-plan` tasks | Tasks share one worktree and no PR; the handoff to `orch_ready_pickup_one` already applies the gate once on the full diff |

## Consequences

- **Positive**: a Ready issue reaches `done` only after an independent reviewer
  approved the diff and a real verify ran; retries see why they failed.
- **Negative**: two extra agent calls per issue (reviewer, verifier), up to four
  when the first review requests changes; repos without any test command now
  block instead of passing.
- **Escape hatch**: the literal `orch: allow-no-verify` in the issue body keeps
  the old `true` behavior for that issue only (harness sets
  `ORCH_ALLOW_NO_VERIFY=1`); `ORCH_VERIFY=<cmd>` still overrides resolution.
  `headless.sh` on a repo without a verify command now exits non-zero unless
  `ORCH_VERIFY` is set.
- **Constraint**: reviewer and verifier never edit, commit, push, or merge.
  Harness tests stubbing an agent must answer `REVIEW: approved` and create
  `.orch/verifier-ok` to reach `done`.

## References

- `skills/core/cda-dev/scripts/orch.sh` (`orch_ready_pickup_one`),
  `lib/verify.sh`, `lib/loop.sh`, `lib/graph.sh`
- `skills/core/cda-dev/prompts/reviewer.md`, `prompts/verifier.md`
- `skills/core/cda-dev/conventions.md` § Ready-pickup flow, § Verify resolution
- Tests: `skills/core/cda-dev/scripts/tests/test_verify_gate.sh`, `test_loop.sh`
- Issues: [#92](https://github.com/chatondearu/ai-dev-kit/issues/92),
  epic [#90](https://github.com/chatondearu/ai-dev-kit/issues/90)
- [[ADR-002-orch-auto-and-fast-lane|ADR-002]],
  [[ADR-005-cda-graph-orchestration|ADR-005]]
