---
name: cda-overwhelm
description: When everything feels urgent, sort the real pile into do-now / schedule / drop / delegate and hand back ONE first action. Builds the pile from evidence (open PRs, in-progress projects, journal "in progress" items) before asking. Use when the user says "everything is urgent", "I'm drowning", "too much on my plate", "I don't know where to start", or lists many competing tasks.
---

# Overwhelm triage

Overwhelm says everything is urgent, important and yours, right now. It rarely is. Turn the swirl into a short, calm list and one thing to do.

> Adapted from `cda-overwhelm` in [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) (MIT), fed from real sources.

## 1. Build the pile (read first)

- **Open PRs:** `gh search prs --author @me --state open --json title,repository,url,updatedAt` (add `--owner $GH_OWNER` when set).
- **Personal sources (optional)** from `${XDG_CONFIG_HOME:-$HOME/.config}/ai-dev-kit/sources.env`: `PROJECTS_DIR/*/index.md` whose `statut:` is active (with their `phase:`), and the latest note in `JOURNAL_DIR` (its "🔄 En cours" and unchecked `- [ ]` items).
- **What the user just dumped.** Merge, dedupe, show the pile in one compact list, then ask only: which deadlines truly can't move, and how much capacity is left today.

## 2. Sort

Test every "urgent" item against a real deadline and a real consequence, then bucket it:

- **🔴 Do now** — real, urgent, and only the user can do it. Keep it to 1–2.
- **🗓 Schedule** — real, not today → give it a *when*.
- **🗑 Drop** — feels mandatory, isn't. Say so explicitly; this is the relief the user can't grant themselves under stress.
- **🤝 Delegate / ask** — someone else's, or shareable → name who.

## Reply

```
Right now, just: <the single first do-now action>
🔴 Do now: …
🗓 Schedule: <item → when>
🗑 Drop: …
🤝 Delegate: <item → who>
Not all of this has to happen today. Most of it doesn't.
```

Never put everything in "do now", never refuse to drop anything, never hand back the list without the single first action.
