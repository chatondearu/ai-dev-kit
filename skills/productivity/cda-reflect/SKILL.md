---
name: cda-reflect
description: A short, warm end-of-task reflection about the user's way of working — one concrete thing they did, one forward-looking "next time try", one specific earned praise, optionally one lesson card — at most 8 lines, no shame. Use on "/cda-reflect", "quick reflection", "wrap up", "reflect on this task", or at the end of a task when the user asked for reflections.
---

# Mini reflection

A light look back at the task that just finished — about the **user's** way of working, not the assistant's, and looking forward, not at mistakes.

> Adapted from `cda-reflect` in [onion-42/nadja-public-claude-skills](https://github.com/onion-42/nadja-public-claude-skills) (MIT).

## Format (≤ 8 lines)

- **🪞 You today:** one concrete thing the user did or decided, and what it says about how they work.
- **💬 Next time try:** quote their real wording or decision → a better move, and why it helps.
- **🌱 Praise:** one specific thing done well — quote the real thing.
- **📝 Card** (optional): one lesson worth a flashcard, only if the session truly produced one. "No card today" is normal.

Then, if anything is unfinished, one `- [ ]` list so nothing is lost between sessions.

## Rules

- Use the real session context; never invent. Typos and grammar are never a finding.
- Don't write files unasked. If the user wants it kept and `JOURNAL_DIR` is set in `${XDG_CONFIG_HOME:-$HOME/.config}/ai-dev-kit/sources.env`, append it to today's note under `## Ce que j'ai appris` (never inside auto-generated blocks).
- Reply in the user's language.
