---
name: standing
description: Where do I actually stand? Evidence-backed weekly progress across every project — shipped work, invisible work, and the window compared with the user's OWN trailing 28 days, every number tied to an instrument and a window. Use when the user asks "how am I doing", "am I making progress", "was this week worth it", feels behind, or compares themselves with others. Never invents a benchmark; says Unknown when it can't measure.
---

# Standing

Answer "am I getting anywhere?" with evidence instead of the imagined peer who ships every day. The baseline is the user's own trailing month; a field comparison appears only when a cited source exists. Accurate positive reinforcement, not comfort: a number the user can't trace gets discounted like the feeling it was meant to counter.

> Adapted from [Citability/claude-code-adhd-skills](https://github.com/Citability/claude-code-adhd-skills) `standing` (MIT), extended from one project directory to every project and repo touched in the window.

## Inputs

| Input | Source | Default |
|---|---|---|
| Window | the user's words | the 7 days ending now |
| Sessions | Claude Code transcripts in `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/projects/*/*.jsonl` modified in the window | all of them |
| Repos | the distinct `cwd` values found in those transcripts that are git repos | — |
| PRs | `gh search prs --author @me --merged` in the window (add `--owner <org>` if the user scopes it) | all orgs |
| Baseline | same instruments over the 28 days before the window start | — |
| Field source | a benchmark file the user points to, each claim with number + source + date + population + window | none → Unknown |

Use full ISO timestamps everywhere (`--since=2026-10-09T00:00:00 --until=2026-10-16T00:00:00`): relative dates reach back an extra evening and bare dates get today's time of day. State the exact bounds next to every window label.

## Protocol

1. **Collect sessions.** `find "$PROJECTS" -name '*.jsonl' -newermt <start> ! -newermt <end>`. Read only `type` user/assistant rows; skip `isSidechain: true` (subagents). Extract with `jq`/`grep` and cap — never load whole transcripts.
2. **Sort the work into four buckets**, each item with date and project:
   - **Shipped** — commits/PRs/deploys, cross-checked against `git -C <repo> log --no-merges --since/--until --format='%h %ad %s' --date=iso` or the PR list. Named in a transcript but absent from git → not shipped; say which.
   - **Written, not committed** — `git -C <repo> status --porcelain` / `diff --stat` now.
   - **Invisible work** — decisions, dead ends ruled out, bugs understood. Transcript-only: quote the line that proves it, or drop it.
   - **Open** — started, not finished, with the last state seen. Incomplete is not failed.
3. **Baseline.** Same instruments over the 28 days before the window, per week, cut on the same weekday; give the median. Derived numbers show their arithmetic ("6 vs median 4.5 → +33%").
4. **Field comparison** only from a cited source, with the population difference stated. Otherwise: "No cited field source, so no comparison to the field — your own trailing month is the benchmark." Never use a remembered statistic.

## Report

Lead with one sentence, then the evidence. Label claims **Verified** (hash, path, command + output, quote), **Inferred** (a reading), **Unknown** (no instrument).

1. **Standing** — one labelled sentence.
2. **Shipped** — hash/PR and date each, grouped by project.
3. **Invisible work** — one quote each.
4. **Window vs baseline** — table: measure · window (dates) · baseline median (dates) · instrument (commits, days with a commit, PRs merged, sessions).
5. **Vs the field** — cited claim or the Unknown sentence.
6. **Open, right-sized.**
7. **One next action**, sized to one session.

A window above baseline is said plainly. Below baseline is located, not excused (fewer days worked, a bigger task, interrupts — the transcripts usually show which). Keep it short: evidence next to the feeling, then stop.

## Never

- Emit a number without its instrument and window — that is this skill failing at its one job.
- Widen the window to rescue a number, or count an assistant's "done" as evidence.
- Argue with the feeling, or turn the report into a plan.

## Limits

Transcripts are per machine and may be pruned (old session counts are often Unknown); git and GitHub are the durable instruments. Work done elsewhere is Unknown, and the report says so.
