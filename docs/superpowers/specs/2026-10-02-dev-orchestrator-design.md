# Design: dev-orchestrator (unified meta-orchestrator)

Date: 2026-10-02  
Status: draft (pending user review)  
Repo: ai-dev-kit

## Problem

The kit has a strong foam + kanban + batch path, but lacks a single portable
engine for the 2026 coding-agent workflow: parallel worktrees, autonomous
verify/CI loops, optional sandbox, and multi-tool survival (Cursor, Claude,
opencode, or any future CLI). `foam-batch-orchestrator` stops at open PRs, only
supports chat multi-task intake, and only *prefers* worktrees without tooling.

## Goals

- One meta-orchestrator: intake → foam/kanban → worktrees → implement → verify
  loop → PR → CI green.
- Agent-agnostic from v1 (Cursor, Claude Code, opencode, `$ORCH_AGENT`).
- Autonomy ends at **CI green + PR mergeable** — never auto-merge.
- Hybrid driver: skill contract + bash harness; skill-only fallback.
- Optional bubblewrap sandbox (off by default).
- Token-efficient: fresh context per loop iteration; minimal master context.

## Non-goals (v1)

- Auto-merge or force-push to main/master.
- Full per-worktree runtime isolation (DB/Docker) — document warnings only.
- ADK-style eval harness, UI dashboard, Windows sandbox.
- Replacing Cursor plugins (Bugbot, security review, loop-on-ci) — compose them.
- Archiving `task-management` in the same change set (follow-up).

## Decisions (locked)

| Topic | Choice |
| ----- | ------ |
| Shape | New skill `dev-orchestrator`; `foam-batch-orchestrator` becomes thin alias |
| Portability | Multi-agent from day one |
| Autonomy | Until CI green; no merge |
| Driver | Hybrid skill + `orch.sh` harness |
| Sandbox | Optional `--sandbox` (bwrap), off by default |
| Entries | `intake` + `ready-pickup` + `pr-fix` |
| Parallelism | Default cap 3 (`ORCH_MAX_PARALLEL`) |
| Loop budget | Default max 10 iterations per worker (`ORCH_MAX_ITER=10`) |

## Architecture

```
dev-orchestrator (skill = contract + phases)
        │
        ├─ modes: intake | ready-pickup | pr-fix
        ├─ GO gate (intake only)
        ▼
orch.sh (agent-agnostic harness)
        │
        ├─ detect: cursor-agent | claude | opencode | $ORCH_AGENT
        ├─ worktree per child (required when parallel)
        ├─ loop: run → verify → retry until green | max-iter | BLOCKED
        ├─ sandbox: --sandbox bwrap (opt-in)
        └─ CI watch until green (no merge)
        ▼
Existing skills: foam-project-memory, foam-prd, foam-plan,
github-kanban-orchestrator, git-commit, code-reviewer (+ host plugins)
```

### Hard rules

1. Master never edits product application source; workers do.
2. One child = one worktree = one branch = one PR.
3. Autonomy stops at CI green + mergeable PR.
4. No auto-merge.
5. Harness uses portable bash/`git`/`gh` only — no proprietary IDE APIs.
6. Do not invent product decisions to unblock; surface BLOCKED.

## Modes

| Mode | Trigger | Human GO | Bootstrap | Exit |
| ---- | ------- | -------- | --------- | ---- |
| `intake` | Multi-subject chat list | Yes (epic breakdown) | Epic issue + approved PRD + N Ready children + N plans | PRs CI-green |
| `ready-pickup` | Ready Kanban cards with PRD/plan | No | None (validate artifacts exist) | PRs CI-green |
| `pr-fix` | PR URL/number | No | None | Same PR CI-green |

## Pipeline

```
Resolve context (repo, foam, board, agent CLI)
  → Ensure artifacts (mode-dependent; GO gate for intake)
  → Ownership check (non-overlapping paths when parallel)
  → Provision worktrees under .orch/worktrees/
  → Dispatch workers (cap ORCH_MAX_PARALLEL, queue overflow)
  → Per worker: implement → local verify → systematic review → open/update PR
  → CI loop (gh pr checks) until green or BLOCKED
  → Aggregate ledger → human report (mergeable PRs; do not merge)
```

### Verify oracle (order)

1. Project verify commands from `AGENTS.md` / nix develop when present.
2. Systematic review (critical/important must be fixed or BLOCKED).
3. Remote CI via `gh pr checks` until green.

Loop iterations use a **fixed prompt** and **on-disk state** (files, git,
ledger). Each iteration should start with a fresh agent context window when the
harness invokes a CLI (Ralph-style), not an unbounded chat transcript.

## Layout

```
skills/core/dev-orchestrator/
  SKILL.md
  conventions.md
  prompts/
    intake.md
    worker.md
    reviewer.md
    pr-fix.md
  templates/
    epic-breakdown.md
    run-ledger.md
  scripts/
    orch.sh
    lib/
      agents.sh
      worktree.sh
      loop.sh
      sandbox.sh
      ci.sh
      verify.sh
```

### Worktrees

- Path: `.orch/worktrees/<issue#>-<slug>/` (add to project `.gitignore` guidance).
- Bootstrap: copy `.env` if present; install/deps via project convention or
  `nix develop -c …`.
- Ports: `ORCH_BASE_PORT + worker_index` recorded in the ledger.
- Cleanup: only after human merge or explicit `--force-clean`.

### Sandbox

- Default off.
- `--sandbox`: bwrap RW=worktree, RO toolchains, hide `~/.ssh` and sibling repos.
- If `bwrap` missing: warn and continue without sandbox (never silent hard-fail).

### Agent detection

Auto order: Cursor agent CLI (if on `PATH`) → `claude` → `opencode` → fail
with install hints. Exact Cursor binary name is resolved at implement time
(`cursor-agent`, `agent`, or documented equivalent) behind one adapter.
Override: `ORCH_AGENT` or `--agent` (full command).
Each adapter: non-interactive flags, `cwd`=worktree, prompt file, stable exit codes.

### Skill-only fallback

If harness missing/unusable or no agent CLI: the skill describes the same
pipeline so the host agent (e.g. Cursor Task) performs worktrees/workers
manually. Same ledger and rules.

## Worker contract

Master injects only:

- mode + ids (issue# and/or PR#)
- plan path, epic PRD path, relevant ADR paths
- worktree path + branch name
- verify command hints
- max-iterations, sandbox on/off

Worker must not: touch another worktree, merge, invent product decisions, skip
verify, or expand scope beyond the approved plan/PRD.

### BLOCKED conditions

ADR conflict, product ambiguity, missing secrets/access, ownership/merge
collision, max iterations reached, CI red that cannot be fixed without a
product/decision input.

## Token economy

- Master holds ledger + ids + paths — not full worker diffs.
- Workers get fresh context per harness iteration.
- `pr-fix` and small `ready-pickup` runs do **not** create epic/PRD noise.
- Keep `SKILL.md` short; put recipes in `conventions.md` / prompts.

## Migration

1. Add `dev-orchestrator` skill + harness scripts.
2. Turn `foam-batch-orchestrator` into a thin pointer to `mode=intake`.
3. Update `project-agents-setup` templates, `rules/01-project-workflow.md`, README.
4. Re-run `install.sh`; clean stale `*.bak-*` skill links (ops note).
5. Follow-up (separate): archive `task-management`; wire or remove `codex/` /
   `vscode/` placeholders.

## Success criteria (v1)

1. Batch of 2 independent subjects → 2 worktrees → 2 PRs → CI green (or explicit BLOCKED).
2. `pr-fix` on a red PR → fix commits → checks green.
3. Same skill runnable from Cursor **or** Claude **or** opencode (harness or fallback).
4. Zero merges without a human action.
5. Optional sandbox path documented and runnable on Linux when `bwrap` exists.

## Open follow-ups (post-v1)

- Per-worktree Docker/DB port isolation helpers.
- Stronger sandbox network policy.
- Explicit `--merge-when-green` gated flag (still default off).
- Skill eval fixtures (golden ledgers / dry-run traces).
