---
name: weekly-unstuck
description: A 10-minute weekly reset — empty the mental backlog and triage it, name the ONE thing for the week, give each stuck item a tiny first step, glance at last week's wins, and run an honest autopilot check. Forward-looking, built from evidence where it exists. Use when the user says "weekly reset", "plan my week", "I feel scattered", "get me unstuck for the week", or at the start/end of a week.
---

# Weekly unstuck

Willpower resets every week; systems don't. A short ritual that does the executive-function work most weeks skip — a reset to run, not a system to maintain.

> Adapted from `weekly-unstuck` in [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) (MIT). Looks forward; for the evidence-backed look back use `standing`.

## 1. Empty the backlog

Pre-fill the pile from what exists, then let the user add to it: open PRs (`gh search prs --author @me --state open`, `--owner $GH_OWNER` when set), and — from `${XDG_CONFIG_HOME:-$HOME/.config}/ai-dev-kit/sources.env` if present — active projects in `PROJECTS_DIR/*/index.md` and the latest `JOURNAL_DIR` note's "🔄 En cours" / unchecked items. Triage it fast: do / schedule / drop / delegate (same rules as `overwhelm-triage`).

## 2. Focus and unstick

- **🎯 The one thing** this week — the highest-leverage focus, so the week has a spine. One, not three.
- **Unstick** — for each stalled item (an active project or PR untouched for 5+ days is a candidate), one tiny first step. Motion over planning.

## 3. Calibrate

- **Wins last week** — 2–3 concrete ones (merged PRs, shipped projects). If the user wants numbers, run `standing`; don't invent them here.
- **Autopilot check** — one honest, gentle nudge: where they're coasting, over-relying on a tool or habit (including the AI), or avoiding something.

## Reply

```
🎯 The one thing this week: <…>
Unstick: <item → first step> (×2–4)
Triage: do <…> · schedule <… → when> · drop <…> · delegate <… → who>
Wins last week: <…>
Autopilot check: <one nudge>
Week's shape: <loose direction, not a schedule>
```
