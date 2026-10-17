---
name: cda-where
description: Rebuild where the user was after an interruption — done / mid-way / next — from evidence (the latest session transcript for this directory, git state, optional project notes), recover the half-thought, and hand back ONE re-entry action. Use when the user says "where was I", "I got interrupted", "I lost my place", "what was I doing", or comes back to a repo after a break.
---

# Where was I

An interruption costs the re-immersion, not the minute. Rebuild the thread from the record instead of asking the user to remember — remembering is exactly what just failed.

> Adapted from `context-switch-recovery` in [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) (MIT), made evidence-first.

## Gather (read, don't ask)

1. **Last session here.** The newest `*.jsonl` in `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/projects/<slug>/`, where `<slug>` is `pwd` with every `/` replaced by `-`. Read only the last ~40 `type: user|assistant` rows, skip `isSidechain: true`; extract the last goal, the last thing completed, and the last open intent. Note its timestamp → how long ago.
2. **Git state.** `git status --porcelain`, `git diff --stat`, `git log -5 --oneline`, current branch, and `gh pr view --json number,state,title` if the branch has a PR.
3. **Personal sources (optional).** If `${XDG_CONFIG_HOME:-$HOME/.config}/ai-dev-kit/sources.env` exists, read its `KEY=value` lines: `PROJECTS_DIR` (one folder per project with an `index.md`) — use the one whose `branche:`/`worktree:` matches this repo; `JOURNAL_DIR` — today's/yesterday's note, for a "🔄 En cours" item or a "je suis là, la suite c'est…" breadcrumb.
4. Anything missing → skip it and say so in one line. Ask the user only for the half-thought, and only if the record doesn't show it.

## Reply

```
Re-enter with: <one concrete action — open X at line N, run Y, answer Z>
You were: <task> · away ~<duration>
Done: <…>   Mid-way: <…>   Next was: <…>
About to lose: <the half-thought / open question, quoted from the record>
Breadcrumb next time: before you leave, drop one line "je suis là, la suite c'est ___".
```

The re-entry action must re-immerse (reopen, rerun, reread the last line), not ask for a fresh decision. Every "done" points at evidence (commit, file, transcript line); no evidence → it goes under "Mid-way" or is dropped.
