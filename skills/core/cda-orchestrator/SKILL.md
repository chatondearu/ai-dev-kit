---
name: cda-orchestrator
description: >-
  Drive interactive cdamux workers from one long-lived orchestrator session:
  spawn/track, relay messages, answer or escalate asks, read reports by ref.
  Use when you are a cdamux orchestrator (`cdamux orch new` or Fleet role
  orchestrator), or when the user asks you to orchestrate workers via cdamux.
---

# cdamux orchestrator

You own one long-lived run. Workers are interactive tmux sessions under you
(`role=worker`, `parent` = your `$CDAMUX_AGENT_ID`). State and messages go only
through the `cdamux` CLI (ADR-011).

## Route the work

| Work | How |
| ---- | --- |
| Kanban issue with a plan or agent brief | `orch.sh` sub-run (below): verify, review gate and CI come with it |
| Feature or fix asked in chat, no issue yet | Skill `cda-dev` intake first (issue, plan or brief), then the row above |
| Research, spike, or trivial fix | `cdamux worker spawn` with a spec (below) |
| A quick lookup or answer | Do it yourself; no worker |

`orch.sh` sub-run: one call per repo with every issue, **in the background**
(Claude Code `run_in_background`; elsewhere `setsid … &`): it runs for long and
kills its workers if cut off. `CDA_DEV` is the `cda-dev` skill folder
(`${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills/cda-dev` in Claude Code).

```bash
bash "$CDA_DEV/scripts/orch.sh" --mode ready-pickup --repo <repo> --issue 12 --issue 15
```

One message per worker (`done` / `blocked` / `error`) and one at run end reach
your inbox.

One worker per disjoint scope. Never start a second worker on the same scope:
`peek` the first one.

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
cdamux worker spawn <name> --prompt-file spec.md [--agent X] [--here]
cdamux worker ls
cdamux worker peek <name> [--lines N]
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

1. `cdamux msg read [--ack] [--digest]` when nudged or after a turn.
2. Inside the spec: `cdamux msg send --to <worker> --kind answer "…"`.
3. A product decision, or outside the spec: ask the maintainer in your chat,
   then answer the worker once they decide.

`kind=ask` leaves the worker `blocked` (Fleet `?`) until an `answer`.

## Stuck workers

`working ⚠` (quiet pane) or a worker shown waiting with no `ask` in your inbox:
`cdamux worker peek <name> --lines 60`. A permission prompt or a question the
spec covers → relay the answer; anything else → escalate to the maintainer.

## Done → ready to merge

**From an `orch.sh` worker**: its PR already passed verify, the review gate and
CI. Report it ready to merge. `blocked` / `error` → read the `ref`, then
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
3. Tell the maintainer the PR is ready to merge. Never merge.
4. After the merge, offer to remove the worktree; never remove one with
   unpushed commits.

On `error`: summarise the cause and the next step for the maintainer.
Read `ref` paths instead of pasting bodies; do not dump JSON. Messages stay
≤ 2 KB (`cdamux msg`).

## Talking to the maintainer

First line: the one thing they need to do or decide (or "nothing to do"). Second
line: the state, one entry per worker: `api ✅ PR #12 · ui ⏳ · db ❓ waiting on you`.

## Out of scope

- Native wake upgrades, Fleet unread glyphs (other slices).
- Installing client CLIs or enabling hooks (ADR-001 / ADR-007).
