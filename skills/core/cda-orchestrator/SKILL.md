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
| Kanban issue with a plan or agent brief | `orch.sh` sub-run from skill `cda-dev` (e.g. `bash <cda-dev>/scripts/orch.sh --mode ready-pickup --issue 12`): verify, review gate and CI come with it; one message per worker reaches you |
| Ad-hoc change or research | `cdamux worker spawn` with a spec (below) |
| A quick lookup or answer | Do it yourself; no worker |

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
3. A product decision, or outside the spec: ask the maintainer. Before you ask,
   mark yourself waiting so Fleet shows it:
   `cdamux state append agents "$CDAMUX_AGENT_ID" '{"status":"blocked"}'`
   (cleared when they reply). Answer the worker once they decide.

`kind=ask` leaves the worker `blocked` until an `answer`.

## Stuck workers

`working ⚠` (quiet pane) or a worker shown waiting with no `ask` in your inbox:
`cdamux worker peek <name> --lines 60`. A permission prompt or a question the
spec covers → relay the answer; anything else → escalate to the maintainer.

## Done → ready to merge

On `done` with a PR:

1. `gh pr checks <url>`: red → relay the failure to the worker once.
2. Review the PR with skill `cda-review` in a **fresh sub-agent**, not in your
   own context. Critical/Important → relay them once; still failing after the
   fix → escalate to the maintainer.
3. Tell the maintainer the PR is ready to merge. Never merge.
4. After the merge, offer to remove the worktree; never remove one with
   unpushed commits.

On `error`: summarise the cause and the next step for the maintainer.
Read `ref` paths instead of pasting bodies; do not dump JSON. Messages stay
≤ 2 KB (`cdamux msg`).

## Talking to the maintainer

Start every message with one state line, one entry per worker:
`api ✅ PR #12 · ui ⏳ · db ❓ waiting on you`. Then the one thing they need to
do or decide.

## Out of scope

- Native wake upgrades, Fleet unread glyphs (other slices).
- Installing client CLIs or enabling hooks (ADR-001 / ADR-007).
