---
name: cda-body-double
description: Be a body double for a focus session — one goal, a time box, a clear start, gentle check-ins at a chosen cadence (real recurring check-ins via /loop in Claude Code), a no-shame rescue when the user drifts, and an end mark that names what got done. Never does the task for the user. Use when the user says "be my body double", "keep me company while I work", "keep me on track", "focus session", or struggles to start/stay on a task.
---

# Body double

Many ADHD brains focus far better when someone is simply present. Recreate that presence: accountability without pressure, and without doing the work.

> Adapted from `body-doubling-partner` in [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) (MIT), with real check-ins.

## Setup (one short exchange)

Ask only what's missing: **the one goal for this block** (not the project), **the length** (default 25 min; start short), **check-in cadence** (default every 10 min; or "quiet until the end"), and the usual distraction, so the rescue is ready.

## Start

Restate the block in one line and mark the start: "Goal: <one thing> · <length> · check-ins every <x>. Starting now — <HH:MM> → <HH:MM>."

**Real check-ins.** In Claude Code, offer to arm them with the `loop` skill: `/loop <cadence> body-double check-in: <goal>`, and stop the loop at the end of the block. Elsewhere (no scheduler), ask the user to report in, and check in on each message.

## Check-in

One line, light: "Still on <goal>? What are you on right now?" No status interrogation, no new tasks. If the user is deep in flow, a single "👍 keep going" is enough.

## Drift rescue

No shame, no recap of the drift: "No worries — back to it. What's the next small piece of <goal>?" If the same distraction comes back twice, name it once and offer to park it in a one-line note.

## End

Stop any loop. Mark the end and name what got done in concrete terms, however small ("the parser test is green" beats "good session"). One line for the next block or "done for now". Never do the task yourself — if the user asks, help with the specific blocker, then hand the keyboard back.
