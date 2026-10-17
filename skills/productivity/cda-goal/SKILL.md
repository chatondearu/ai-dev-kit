---
name: cda-goal
description: Pin a dense or multi-part request as an explicit GOAL block (primary goal, sub-goals with their target artifact, hard vs soft constraints, ambiguities as multiple choice, definition of done), then run a drift check when later messages narrow or add scope. Use when the user says "cda-goal", "pin the goal", "what are we doing", "don't lose the thread", pastes a long prompt with several bundled asks, or when a long session starts drifting.
---

# Goal

Write down what the task *is* before working on it, so the page remembers instead of working memory. This skill does not do the task — it pins it, and re-checks it when scope moves.

> Adapted from [onion-42/nadja-public-claude-skills](https://github.com/onion-42/nadja-public-claude-skills) `cda-goal` (MIT).

Skip it for a short, single, unambiguous ask.

## Pin

1. Read the request in full; keep the user's own terms, paths and thresholds — don't paraphrase meaning away.
2. **Primary goal** — one sentence: the outcome that makes the rest make sense.
3. **Sub-goals** — a numbered `[ ]` list. For each, name the **target artifact** (function, file, cell, issue) that will change. If the request doesn't pin it down, write `TARGET: unpinned` and make it the first question.
4. **Constraints** — HARD (violating it = failure) vs SOFT (tradeable). If nothing is soft, say "none stated"; don't invent softness.
5. **Open questions** — only ambiguities that change the work, as 3–4 concrete options plus "something else".
6. **Definition of done** — an observable artifact or state (a file, a passing check, a number), not a feeling.

```
GOAL [pinned — re-read before continuing]
Primary: <one sentence>
Sub-goals:
1. [ ] <…> — target: <artifact | unpinned>
2. [ ] <…>
Hard: <… | none stated>
Soft: <… | none stated>
Open questions: <Q1 A/B/C/D | none>
Done when: <observable state>
```

## Drift check

When resuming, or when a message may shift scope:

1. Re-read the last pinned GOAL, then the messages since.
2. One line per sub-goal: **done** / **in progress** / **not started**.
3. New asks → `NEW (added mid-session): …`. A redefinition → `NARROWED: sub-goal N now means …`. Never swap scope silently.
4. Re-emit the GOAL block if anything changed.

An unpinned target and an unmeasured assumption are what cost a full round trip — catch both here, before the work.
