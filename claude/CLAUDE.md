# Global instructions (Claude Code)

Auto-generated from `rules/*.md` in **ai-dev-kit**. Do not edit this assembled
file by hand — change files under `rules/` and run:

```bash
./scripts/assemble-claude-md.sh
./install.sh --claude
```

Portable skills are symlinked into `~/.claude/skills/`. Per-repository guidance
lives in each project's `AGENTS.md` (bootstrap with skill `cda-agents`).

---

# Communication and code style

## Language

- Reply to the user in **French**.
- Write **code comments in English**.
- Write **foam, ADR, PRD, plan, and user-facing technical docs in English** unless the project explicitly states otherwise.

## Vue / Nuxt (when the project uses them)

- Use **Composition API** with `<script setup>`.
- Type component **props** explicitly.
- Follow **Anthony Fu** ESLint and style guide conventions.

## Commits and PRs

- Use **Conventional Commits** (`feat:`, `fix:`, `docs:`, `refactor:`, etc.).
- Only create commits when the user asks.
- Use `gh` for GitHub operations (issues, PRs, checks).

## Code principles

- Minimize scope — smallest correct diff.
- Match existing project conventions before introducing new patterns.
- Add comments only for non-obvious business logic.
- Add tests only when requested or when they cover meaningful behavior.

---

# Project workflow (foam + kanban)

In **every** repository you work on:

1. **Bootstrap** if missing: run `cda-agents/scripts/setup-agents.sh`
   (and `--init-foam` when `foam/` does not exist).
2. **Before** planning or implementing a feature: read `AGENTS.md`, then
   `foam/index.md`; check Kanban issues and open PRs for overlap.
3. **Before** any technical, product, or design decision that affects the
   project: search existing ADRs, foam notes, GitHub issues, and PRs; read
   relevant material and **confront** the new proposal with recorded decisions.
4. **During** work: keep PRD/plan/ADR artifacts aligned (`cda-foam`,
   `cda-kanban`).
5. **After** shipping or deciding: update foam (ADR, PRD status, feature lists)
   and `AGENTS.md` when stack, skills, or protocols change.

Skills (ai-dev-kit): `cda-foam`, `cda-prd`, `cda-plan`, `cda-dev`,
`cda-kanban`, `cda-agents`, `git-commit`, `code-reviewer`.

---

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

---

# Nix development shell (global user rule)

Copy the block below into **Cursor Settings → Rules → User Rules** if it is not already applied via the `nix-develop-shell` skill.

---

Before running project commands (install, build, test, lint, format, scripts, etc.) in a workspace:

1. Check if the project root has `flake.nix` or `.envrc` / `.envrc.local` (direnv).
2. If present, run commands through `nix develop -c <command>` (or confirm direnv already loaded the environment).
3. Do not assume `pnpm`, `npm`, `node`, `python`, etc. exist on the host PATH outside the dev shell.

Examples:

- `nix develop -c pnpm install`
- `nix develop -c pnpm test`
- `nix develop -c bash -c 'export CI=true && pnpm check'`

For compound commands, wrap the full script: `nix develop -c bash -c '...'`.

If a command fails with `command not found`, retry inside `nix develop` before other fixes.
