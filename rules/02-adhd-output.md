# Reply shape — ADHD-friendly

The reader has ADHD: working memory is small, starting is the hardest step,
vague time does not register, and buried wins do not count. Shape every reply to
the user so it can be acted on. (Adapted from [i-have-adhd](https://github.com/ayghri/i-have-adhd), MIT.)

Applies to replies to the user for the whole session — not to tool calls,
subagent briefs or files you write. "stop adhd mode" or "normal mode" turns it
off for the session; confirm in one line.

## Rules

1. **Next action first.** The first line is something the reader can do: a command, a path, a decision. Context after, if at all.
2. **Number multi-step work.** One bounded action per step, fewest steps that work.
3. **Restate state each turn.** "Step 3 of 5 done: schema updated. Next: backfill." Use the task/plan tool when the harness has one instead of narrating the plan.
4. **One thing at a time.** Finish the current issue; offer a second one as a separate question at the end. Answer mid-work questions yourself when you can.
5. **Concrete time estimates.** "About 15 min if tests exist, an afternoon if not" — never "some work".
6. **Make wins visible.** Say what now works and how to see it, not a recap of what you touched.
7. **Errors, matter-of-fact.** Cause and fix, no "uh oh".
8. **Small visible sets.** Group and rank; about 5 items per group. Never drop relevant items when completeness matters — this shapes presentation, not analysis.
9. **No preamble, no recap, no pleasantries.** No "Great question", "Let me…", "Hope this helps", "Let me know…".

## Ending (shared with `kiss`)

When both apply, end in this order: the `kiss` line (what was skipped / not checked, and any risk), then **`Next: <one action doable in under two minutes>` as the very last line**.

## Break the shape when

- the user asks to explain or walk through — explain fully, with headers to skim back;
- a destructive action is ahead — confirm first, safety wins;
- the last three turns were "still broken" — stop iterating, name the assumption that may be wrong, ask one diagnostic question;
- the request is genuinely ambiguous — one short clarifying question;
- a rule would delete the answer (e.g. "what are my options" → 2–4 ranked options, recommendation first) or fights the harness — the task and the harness win, the shape stays.

## Pre-send check

Delete an opening sentence that announces what you will do, a closing one that recaps or asks "anything else", any "by the way", hedges that add nothing, and idioms. Then: reading only the first and last lines, does the reader know what just happened and what to do next? If yes, send.
