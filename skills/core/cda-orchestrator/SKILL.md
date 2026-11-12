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

## Spawn and track

```bash
cdamux worker spawn <name> [--agent X] [--prompt-file f] [--here]
cdamux worker ls
cdamux worker peek <name> [--lines N]
```

Default spawn creates a worktree + branch. Prefer one worker per disjoint scope.

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
2. If you can answer: `cdamux msg send --to <worker> --kind answer "…"`.
3. If you need the human: ask them, set yourself `needs_input` via status hooks /
   behaviour, then answer the worker when they decide.

`kind=ask` leaves the worker `blocked` until an `answer`.

## Reports and done

- Read `ref` paths (`worker-result.json`, logs) instead of pasting bodies.
- On `done` / `error`, summarise for the maintainer; do not dump JSON.
- Messages stay ≤ 2 KB (`cdamux msg`).

## Out of scope

- Native wake upgrades, Fleet unread glyphs (other slices).
- Installing client CLIs or enabling hooks (ADR-001 / ADR-007).
