# PLAN-417 — fix(cdamux): orch stays a dispatcher

- **Status**: draft
- **PRD**: waived. Issue [#417](https://github.com/chatondearu/ai-dev-kit/issues/417) is the contract. This change is the plan and the graph only.
- **GitHub**: [#417](https://github.com/chatondearu/ai-dev-kit/issues/417)
- **Date**: 2026-11-22
- **Audit**: `/tmp/orch-kiss-audit.md` (also `$CDAMUX_HOME/global/reports/orch-kiss-audit.md`). Not in git.

## Summary

The orchestrator pane dispatches. It does not edit, verify, or debug a repo.
Findings 1–7 of the 2026-11-22 audit are the work. Implementation is later
workers (`orch.sh --graph-plan`). This plan does not change cdamux behaviour.

Skill retro is already [PR #418](https://github.com/chatondearu/ai-dev-kit/pull/418)
(`cda-orch-improve`). Do not recreate that skill. Link it from the orchestrator
skill if the pointer is not on the branch yet, and do not delete it.

KISS-off for `CDA_ROLE=orchestrator` is already
[PR #419](https://github.com/chatondearu/ai-dev-kit/pull/419). Do not open a
second PR for the same two files.

Fil's ring (audit finding 3) is [neomind #226](https://github.com/chatondearu/neomind/issues/226). Not this plan.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry]] | A pane whose process is a shell, `mise`, or `direnv` for 10s is dead: status `error`, parent inboxed. No watcher. Restore is `cdamux restore`, not tmux-resurrect. |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model]] | One inbox. Idle nudge is a one-line paste after the quiet window. No `cdamuxd` watcher. `done` / `error` stick until the next `working`. |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators]] | `[orchestrator].max_workers` is checked at `worker spawn` before any worktree. |
| [[../foam/decisions/ADR-016-foam-first-kanban]] (proposed) | Foam board stays the only board. `skills/` and `cdamux/` are outside the meta-merge allowlist. The maintainer merges. |

## Wake-up decision

**Chosen: drain on Cursor `stop`.** Not a timer, and not a `postToolUse` paste.

Mail that arrives while the orchestrator is `working` is already in
`$CDAMUX_HOME/inbox/<id>.jsonl` (`append_message` is synchronous). Cursor `stop`
already runs `cdamux/hooks/status.js`, and `unreadDigest` already reads that
file. So a stop-drain **can** see mail that arrived during the turn. A periodic
check adds nothing for that case.

On that stop, when the turn has ended and the pane is idle, if unread
`done` / `ask` / `error` / `report` remain, surface one line
(`📨 N messages — cdamux msg read --ack`) and still run the say drain. A turn
that is still `working` is not interrupted.

**Why the other option is worse.** `postToolUse` already calls `status.js` and
only lifts `idle` back to `working` mid-turn. An idle-only drain there cannot
see mail that stop cannot already see, and a paste during tools collides with
the live turn (ADR-011). A new timer process is the watcher ADR-011 rejected.

**Limit, written into the orchestrator skill by task `relay-inbox`:** a long
`working` turn does not see mail until it stops. That is acceptable when stop
is reliable. A turn that never stops (CLI gone, pane left on fish) is task
`reap-comm`, which marks the pane `error`. It is not a reason to add a daemon.

Today Cursor `stop` with unread mail sends `followup_message`, forces the
status to stay `working`, and **skips** `drainSay`
(`cdamux/hooks/test.sh` asserts both). While worker mail keeps arriving, a
queued `cdamux say` never drains. That is the hole the stop task closes.

## Scope

### In scope

Audit findings 1, 2, 4, 5, 6, 7, plus 8, 9, 12, and 13 folded into those
tasks. The wake-up decision above.

### Out of scope

- Finding 3 (Fil ring): neomind #226.
- Finding 10 (`state gc` of `blocked` / `working` files with no host).
- Finding 11 (two console binaries).
- Finding 14 (tmux-resurrect rewriting panes to fish). Leave ADR-009.
- Finding 15 (`AGENTS.md` skills table). The managed block is not edited here.
- Deleting worktrees. The hint prints the command. A human or a worker runs it after merge.
- A second board, a second status, a say-queue for workers.

## Tasks

Graph ids are in [graph/PLAN-417-tasks.json](graph/PLAN-417-tasks.json).
Paths do not overlap, so a later `--graph-plan` can run them in parallel.
`reply-trace` and the wake-up share Cursor `stop` and `cdamux/hooks/test.sh`,
so they are **one** graph node (`stop-hook`). Splitting them would make each
worker fail the other's tests.

- [ ] **kiss-off** (findings 1 and 8) — graph `kiss-off`. When `CDA_ROLE=orchestrator`, the session hook injects `ORCH_ROUTING` only: no KISS ruleset, no level persist, no repo map. Acceptance: hook output contains `cdamux orchestrator` and does not contain `KISS MODE ACTIVE`. A non-orchestrator session still gets KISS. **Land [PR #419](https://github.com/chatondearu/ai-dev-kit/pull/419). Do not open another PR on these files while it is open.**
- [ ] **reply-trace** (findings 2 and 9) — graph `stop-hook`. A Cursor `stop` with no assistant prose still appends one visible `reply` (the turn was tool-only). A 1s hook timeout must not leave the journal empty. Acceptance: one hook test with an empty `last_assistant_message` and no transcript greps a journal line or a non-zero log line.
- [ ] **stop-drain** (wake-up, decision above) — graph `stop-hook`. On Cursor `stop`, when the pane is idle, unread `done` / `ask` / `error` / `report` produce one nudge line, and `drainSay` still runs. A `working` turn is not pasted. Acceptance: the hook test that today forbids `msg drain` on a Cursor followup and requires status `working` is updated so a queued `say` drains on that stop, and `postToolUse` still does not drain.
- [ ] **reap-comm** (finding 4) — graph `reap-comm`. `/proc/<pane_pid>/comm` of `fish`, `mise`, or `direnv` means the agent is dead even when `#{pane_current_command}` is still `cursor-agent`. Existing reap then sets `error` and inboxes the parent. No daemon. Acceptance: a fixture pane whose command string is `cursor-agent` and whose comm is `fish` is reaped after the 10s quiet window; a real `cursor-agent` comm is not.
- [ ] **cap-hint** (finding 5) — graph `cap-hint`. An `idle` host with no pending ask does not count once the quiet window (10s) has passed. A live `done` host already does not count; keep that, and print the same reap hint for it. `working`, `blocked`, and idle-with-a-pending-ask still count. `worker ls` prints `tmux kill-session -t <host>` and the worktree path. It does not kill the session and does not delete the worktree. Acceptance: the count unit test covers idle-past-window (excluded) and idle-with-pending-ask (still counted); `worker ls` output contains the kill hint and the path.
- [ ] **relay-inbox** (findings 6 and 13) — graph `relay-inbox`. A `relay` to a worker that was not pasted prints `inbox`, not `queued`. `queued` remains the orchestrator `say` queue only. The orchestrator skill states both: `inbox` means peek, the worker will not start; a `working` turn does not see mail until Cursor `stop`. Do not remove the `cda-orch-improve` pointer from #418. Acceptance: `delivery_word` test expects `inbox` for an unpasted worker send; the skill contains those two sentences.
- [ ] **here-disk** (findings 7 and 12) — graph `here-disk`. `worker spawn --here` refuses when the branch is `main` or `master`, or the tree is dirty. The error is one line and no session is created. `git worktree add` is bounded: timeout or ENOSPC (`No space left on device`) returns one line and does not hang. A new worktree (`worker/<run>/<name>`, clean) and an orchestrator on `main` still spawn. Acceptance: `orch_tmux.rs` keeps one clean non-main `--here` success and gains the refusal; a worktree-add failure is a single error line.

## Graph

| Id | Owns | Do not touch |
| --- | --- | --- |
| `kiss-off` | `skills/quality/cda-kiss/hooks/activate.js`, `skills/quality/cda-kiss/hooks/test.sh` | cdamux |
| `stop-hook` | `cdamux/hooks/status.js`, `cdamux/hooks/ledger.js`, `cdamux/hooks/test.sh`, `cdamux/crates/cdamux-core/src/events.rs` | orchestrator skill, `msg.rs` |
| `reap-comm` | `cdamux/crates/cdamux-core/src/tmux.rs`, `cdamux/crates/cdamux-core/src/restore.rs` | `worker.rs` |
| `cap-hint` | `cdamux/crates/cdamux-core/src/worker.rs`, `cdamux/crates/cdamux-cli/src/main.rs`, `cdamux/crates/cdamux-cli/tests/worker_cli.rs` | `spawn.rs`, `worktree.rs` |
| `relay-inbox` | `cdamux/crates/cdamux-core/src/msg.rs`, `skills/core/cda-orchestrator/SKILL.md` | `status.js`, `say.rs` |
| `here-disk` | `cdamux/crates/cdamux-core/src/worktree.rs`, `cdamux/crates/cdamux-core/src/spawn.rs`, `cdamux/crates/cdamux-core/tests/orch_tmux.rs` | `worker.rs` |

Do not run `orch.sh` from the plan PR. Each implementation PR tracks #417 and
does not `Closes` it.

## Risks

| Risk | Mitigation |
| ---- | ---------- |
| #418 and #419 edit files this graph also names | `kiss-off` stops if #419 is open or merged. `relay-inbox` keeps the #418 skill pointer. |
| Stop-hook is one worker, not two | The hook test file locks both behaviours. Two writers would fail each other's tests. |
| `cap-hint` owns `main.rs` and `here-disk` owns `spawn.rs` | Prompts limit each worker to the ls hint and the `--here` / worktree-add error. No other CLI change. |
| `--here` check in `spawn.rs` refuses a worker whose cwd is `main` | New worktrees are not on `main`. Orchestrator role is exempt. The nested-worker test that uses `here: true` on `git init` must move to a clean side branch. |
| Cap excludes idle hosts that are about to take a relay | Only after 10s, and not when a pending ask exists. |

## Test plan

Seams, not internals:

- KISS session hook stdout (`skills/quality/cda-kiss/hooks/test.sh`).
- cdamux status hook stdout and the `cdamux` argv it spawns (`cdamux/hooks/test.sh`).
- `record_reply` journal line (`events.rs` unit test).
- `pane_is_dead_agent` / pane parse (`tmux.rs`, `restore.rs`).
- `count_live_workers` and `worker ls` text (`worker.rs`, `worker_cli.rs`).
- `delivery_word` (`msg.rs`).
- `worker spawn --here` and `worktree_add_new_branch` error strings (`spawn.rs`, `worktree.rs`, `orch_tmux.rs` on a throwaway tmux server).

## Verification

This plan PR:

- [x] `bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh plans/graph/PLAN-417-tasks.json` (paths disjoint, 6 tasks)
- [x] `bash scripts/verify.sh` (host `cargo` on `PATH`; exit 0)
- [x] Diff is the plan and the graph JSON only

Implementation PRs (later workers): the task acceptance above, then
`bash scripts/verify.sh`. Do not merge from a worker.
