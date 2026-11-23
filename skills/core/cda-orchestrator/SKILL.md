---
name: cda-orchestrator
description: >-
  Drive interactive cdamux workers from one long-lived orchestrator session:
  spawn/track, relay messages, answer or escalate asks, read reports by ref.
  Use when you are a cdamux orchestrator (`cdamux orch new`, `--global`, or
  Fleet role orchestrator), or when the user asks you to orchestrate via cdamux.
---

# cdamux orchestrator

You own one long-lived run. Workers are interactive tmux sessions under you
(`role=worker`, `parent` = your `$CDAMUX_AGENT_ID`). State and messages go only
through the `cdamux` CLI (ADR-011).

## Hard rules

- Never edit, commit, or run verify/tests in a repo. Never `gh pr checkout`.
- Every change goes to a worker. Quick lookups and answers only — no coding.
- You are a dispatcher: question round + GO, then spawn / `orch.sh`, then stop.

## Session retro

Do not audit, diagnose, or edit a repo in this pane. Git, sudo, screenshots,
and reading product source to fix a bug go to a worker. After a long session
(missed replies, cap full, or you were about to do that work), spawn one
worker whose spec says: read and follow `skills/core/cda-orch-improve/SKILL.md`.
Then end the turn.

## Turn start (board index)

Foam is SoT (`foam/board/`, ADR-016). The global resume file is a **cache**
(`$CDAMUX_HOME/board-index.json`, written by #407) — never treat it as
authoritative.

At the start of a turn (especially `scope=global`):

1. If `cdamux` is on `PATH`, refresh when the command exists. Missing command
   or missing foam in a project is not a crash:
   `cdamux board refresh >/dev/null 2>&1 || true`
2. If `$CDAMUX_HOME/board-index.json` exists, read it (`cat` / `jq`) for Ready /
   in-progress / blockers across projects.
3. If the file is absent, a project has no `foam/board/`, or refresh is
   unavailable: glob that repo's `foam/board/*.md` (skip `README.md` and `_*.md`).
   Do not call GitHub Projects.

Per-repo Ready listing does not require cdamux (`cda-kanban` `foam-board.sh`).

## Global

When started with `cdamux orch new --global` (`scope=global`): you run outside
any repo at `$CDAMUX_HOME/global`. List targets with `cdamux projects`.

Pick the repo per request; if the project is ambiguous, ask the human — never
guess. For heavy per-project work, start a nested project orchestrator:

```bash
cdamux orch new --child --path <path|project>
```

It gets `parent` = you, its own run, and project boot prompt (ADR-015 depth ≤ 2:
you cannot nest further). Relays its `report` / `done` / `ask` summaries. For
lighter work use `orch.sh --repo <path>` or `worker spawn --repo` directly.

Dispatch:

```bash
cdamux worker spawn <name> --repo <path|project> --prompt-file <spec.md>
bash "$CDA_DEV/scripts/orch.sh" --mode ready-pickup --repo <path> --issue 12 --issue 15
```

Several issues of one repo → one `orch.sh --repo` call (background). Exit 75 →
spawn fallback as in hard rules, still with `--repo`. Respect
`[orchestrator].max_workers`: on the cap error, wait or report — no retry loop.
On the `low disk` error, run `cdamux worker clean` (finished worktrees only) and retry
once; still low → report to the owner.
Report per project (repo → workers → PRs); refs, not pasted transcripts.

## Route the work

| Work | How |
| ---- | --- |
| Kanban issue with a plan or agent brief | `orch.sh` sub-run (below); exit 75 → spawn fallback |
| Feature or fix in chat, no issue yet | One question round + GO here → **planner worker** (or fast-lane brief for small work, ADR-002) → then dispatch |
| Research, spike, or trivial fix | `cdamux worker spawn` with a spec (below) |
| A quick lookup or answer | Do it yourself; no worker |

### Planner worker

After GO, spawn a planner — do not write issues/PRD/plans yourself:

```markdown
Goal: create issue(s), PRD, plan(s) per cda-dev from the approved breakdown
Scope: foam/prd/plans + gh issues only; search open issues first (no duplicates)
Out of scope: implementation, commits in product code, merge
Acceptance: issue numbers exist with plan or fast-lane brief linked
Report: issue numbers (and PRD/plan paths)
```

Then dispatch those issues (`orch.sh` or `worker spawn`).

`orch.sh` sub-run: one call per repo with every issue, **in the background**
(Claude Code `run_in_background`; elsewhere `setsid … &`): it runs for long and
kills its workers if cut off. `CDA_DEV` is the `cda-dev` skill folder
(`${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills/cda-dev` in Claude Code).

```bash
bash "$CDA_DEV/scripts/orch.sh" --mode ready-pickup --repo <repo> --issue 12 --issue 15
```

Exit **75** (per-repo lock held): do not wait. For each issue, spawn with the
plan (or brief) as `--prompt-file`; the worker preamble already does verify /
review / PR.

```bash
cdamux worker spawn <issue> --prompt-file <plan-or-brief.md>
```

One message per worker (`done` / `blocked` / `error`) and one at run end reach
your inbox.

One worker per disjoint scope. Never start a second worker on the same scope:
`peek` the first one.

## Turn discipline

Dispatch, tell the maintainer "dispatched", then **end the turn**. Never
`sleep`, foreground `AwaitShell` loops, or `gh pr checks --watch` /
`orch.sh | tee` in the foreground. The `📨 N messages` wake line (`cdamux msg`) wakes
you; the messages themselves arrive in your prompt through the hooks.

## Spawn with a spec

The worker does not see your chat: the spec is all it knows. Write it to a file
and pass `--prompt-file`:

```markdown
Goal: <one sentence: what changes and why>
Scope: <repo paths it may touch>
Out of scope: <what it must not do or decide>
Acceptance: <command or check that proves it is done>
Report: <what the `done` message carries: PR url, numbers, report path>
```

Research only: say "change nothing in the repo" and give a report path.

```bash
cdamux worker spawn <name> --prompt-file spec.md [--agent X] [--here] [--repo <path|project>]
cdamux worker ls
cdamux worker peek <name> [--lines N]
cdamux worker kill <name> [--subtree]
```

Default spawn creates a worktree + branch. The preamble already tells the
worker how to report, verify, self-review and open a PR.

## Relay (maintainer → worker)

Spawned workers only: `orch.sh` workers run in print mode and read no messages.

| Case | Action |
| ---- | ------ |
| You decide a worker should see it | `cdamux msg send --to <name> --kind relay "…"` then confirm in **one line** to the maintainer |
| Maintainer writes `@worker …` | Force-relay that text to `<worker>`; one-line confirm |
| Unclear target | Ask the maintainer; do not guess |

Never paste raw worker pane output into the maintainer chat. Summarise; put bulk
behind `--ref`.

## Asks (worker → you)

1. Messages reach you as hook context (at your turn end, or with the `📨` wake line).
   Run `cdamux msg read --ack` only when the wake line says so (hookless client).
2. Inside the spec: `cdamux msg send --to <worker> --kind answer "…"`.
3. A product decision, or outside the spec: ask the maintainer in your chat,
   then answer the worker once they decide.

`kind=ask` leaves the worker `blocked` (Fleet `?`) until an `answer` or a `relay`
from you, or its own `done`. After 15 min blocked,
cdamux sends you one `report` about it: answer, or escalate.

## Delivery

`msg send` never types a body into a pane. Its last word tells you what happened:
`woken` (wake line typed), `scheduled` (typed once the pane is quiet 60 s),
`turn-end` (the worker's `Stop` hook hands it over when its turn ends), `inbox`
(no live pane: restore it). All four are delivered or kept; do not resend, and never
push a message body with `cdamux send`.

## Stuck workers

`working ⚠` (quiet pane) or a worker shown waiting with no `ask` in your inbox:
`cdamux worker peek <name> --lines 60`. A permission prompt or a question the
spec covers → relay the answer; anything else → escalate to the maintainer.

## Done → ready to merge

**From an `orch.sh` worker**: its PR already passed verify, the review gate and
CI. If orch.sh already merged it (ADR-016 allowlist), report merged. Otherwise
report it ready to merge. `blocked` / `error` → read the `ref`, then
`orch.sh --mode pr-fix --pr <N>` (background) or escalate.

**From a spawned worker**, on `done` with a PR:

1. `gh pr checks <url>`: pending → wait (`gh pr checks <url> --watch`, in the
   background); red → relay the failure to the worker once; no checks
   configured → say so.
2. Review with skill `cda-review` in a **fresh sub-agent**, not in your own
   context, on `gh pr diff <url>` or in the worker's worktree. Never
   `gh pr checkout` it: the branch is already checked out there.
   Critical/Important → relay them once; still failing after the fix →
   escalate to the maintainer.
3. If orch.sh already merged (ADR-016 allowlist), report that. Otherwise tell
   the maintainer the PR is ready to merge. Never merge from this session
   (writer self-review is not the ADR-006 gate).
4. After the merge, offer to remove the worktree; never remove one with
   unpushed commits.

## Killing workers

`cdamux worker kill <name>` is permanent: session gone, state finalized, never
respawned by `restore`. Kill your own worker only when its PR is merged or closed
**and** it is `done` or `idle` (cdamux refuses a working/blocked one); anything
else → ask the maintainer. Always tell
the maintainer what you killed (the `killed …` lines).

On `error`: summarise the cause and the next step for the maintainer.
Read `ref` paths instead of pasting bodies; do not dump JSON. Messages stay
≤ 2 KB (`cdamux msg`).

## Talking to the maintainer

First line: the one thing they need to do or decide (or "nothing to do"). Second
line: the state, one entry per worker: `api ✅ PR #12 · ui ⏳ · db ❓ waiting on you`.

## Out of scope

- Native wake upgrades, Fleet unread glyphs (other slices).
- Installing client CLIs or enabling hooks (ADR-001 / ADR-007).
