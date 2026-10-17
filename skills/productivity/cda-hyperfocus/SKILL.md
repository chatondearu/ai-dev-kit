---
name: cda-hyperfocus
description: A gentle, structured off-ramp from hyperfocus — the real session duration from transcript timestamps, what got neglected, a save-your-place breadcrumb so stopping isn't losing the thread, a clean break point, and a push toward the actual need (food, sleep, people, the forgotten commitment). Use when the user says "I've been at this for hours", "help me stop", "I can't pull away", "one more thing" late at night, or a session has run very long.
---

# Hyperfocus exit

For an ADHD brain, stopping is the hard part — mostly from fear of losing the thread. Make leaving safe: save the place, find a clean break, bridge to the real need.

> Adapted from `cda-hyperfocus` in [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) (MIT).

## 1. Reality check (measured, not guessed)

Duration = first to last `timestamp` of today's rows in the transcripts under `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/projects/*/` (this session plus any others today), with gaps over 30 min counted as breaks. Also the local time (`date +%H:%M`). Name it plainly and without shame: "~4 h 40 straight, it's 21:15." Ask once what got neglected (food, water, sleep, people, a commitment) if it isn't obvious.

## 2. Save the place

Write one breadcrumb so tomorrow's `cda-where` is instant: *done / mid-way / next step / the open question*, two to four lines. If `JOURNAL_DIR` is set in `${XDG_CONFIG_HOME:-$HOME/.config}/ai-dev-kit/sources.env`, append it to today's note under `## À faire demain` (create the heading if missing; never touch auto-generated blocks); otherwise show it in chat for the user to keep. If there's uncommitted work worth keeping, suggest a WIP commit — don't make it.

## 3. Clean break

Pick the end of the smallest unit in flight ("finish this test, then stop") — not an abrupt yank the brain will resist, not "one more feature".

## Reply

```
What you need now: <eat / drink / move / sleep / the forgotten thing>
You've been deep in <task> for ~<duration measured> (it's <time>). Neglected: <…>
Place saved: <where the breadcrumb went>
Clean break: finish <small unit>, then stop.
It'll be right here — tomorrow, `/cda-where` picks it up.
```
