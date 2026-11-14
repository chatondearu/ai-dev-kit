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
- On any non-default branch (feature/fix branch, worktree), commit and push
  freely and incrementally as you progress — no need to ask.
- On the default branch (`main`, `master`, or the repo's default such as
  `develop`), never commit or push without explicit user approval — create a
  branch instead.
- Never force-push a shared branch or rewrite published history without asking.
- Use `gh` for GitHub operations (issues, PRs, checks).

## Code principles

- Minimize scope — smallest correct diff.
- Match existing project conventions before introducing new patterns.
- Add comments only for non-obvious business logic.
- Add one small test (or assert-based self-check) for every non-trivial new logic (branch, loop, parser, money, security); trivial changes need none (`cda-kiss`).

## Exception: cda-dev workers

When the prompt starts with "You are a cda-dev worker" (or is a cda-dev
pr-fix/worker prompt), write code, comments, commit and PR text in **English**,
and follow the worker prompt over the rules above.

---

# Project workflow (foam + kanban)

In **every** repository you work on:

1. **Bootstrap** if missing: propose `cda-agents/scripts/setup-agents.sh`
   (and `--init-foam` when `foam/` does not exist) and wait for the user's OK
   before creating or committing `AGENTS.md` / `foam/`.
2. **Before** planning, implementing, or any technical, product, or design
   decision: follow the foam protocol (`cda-foam` § Protocol) — read memory,
   search ADRs and GitHub issues/PRs, **confront** the proposal.
3. **Chat requests** go through `cda-dev` intake, except a trivial fix (typo,
   one file and about 20 lines, no product decision): branch → commit → PR.
   Several Ready issues or parallel work → `cda-dev` ready-pickup (`orch.sh`,
   graph: `cda-graph`). When unsure, use intake.
4. **During** work: keep PRD/plan/ADR artifacts aligned (`cda-foam`,
   `cda-kanban`).
5. **After** shipping or deciding: update foam (ADR, PRD status, feature lists)
   and `AGENTS.md` when stack, skills, or protocols change.

Skills (ai-dev-kit): `cda-foam`, `cda-prd`, `cda-plan`, `cda-dev`,
`cda-kanban`, `cda-agents`, `cda-commit`, `cda-review`.

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

## Ending (shared with `cda-kiss`)

When both apply, end in this order: the `cda-kiss` line (what was skipped / not checked, and any risk), then **`Next: <one action doable in under two minutes>` as the very last line**.

## Break the shape when

- the user asks to explain or walk through — explain fully, with headers to skim back;
- a destructive action is ahead — confirm first, safety wins;
- the last three turns were "still broken" — stop iterating, name the assumption that may be wrong, ask one diagnostic question;
- a single word or term is ambiguous in an otherwise clear request — one short clarifying question; anything wider goes through `cda-dev` intake, which questions via `cda-grill`;
- a rule would delete the answer (e.g. "what are my options" → 2–4 ranked options, recommendation first) or fights the harness — the task and the harness win, the shape stays.

## Pre-send check

Delete an opening sentence that announces what you will do, a closing one that recaps or asks "anything else", any "by the way", hedges that add nothing, and idioms. Then: reading only the first and last lines, does the reader know what just happened and what to do next? If yes, send.

---

# Graphify code graph (opt-in)

When `CDA_GRAPHIFY=1` and `<repo>/graphify-out/graph.json` exists:

- Read `graphify-out/GRAPH_REPORT.md` (or query the graph) **before** grepping
  or reading source files to orient.
- Do **not** install Graphify from the kit (`ADR-001`); the host / `aitools`
  owns that.
- When the env is unset or the graph file is missing, behaviour is unchanged.

---

# Nix development shell (global user rule)

**Default: do not use `nix develop`.** It saturates disk when agents wrap every
command. Follow skill **nix-develop-shell** (ADR-013):

1. Run bare on host PATH / direnv / activated Mise first.
2. Never wrap `git`/`gh`/OS tools just because `flake.nix` exists.
3. Only after `command not found` (or wrong version) for a **toolchain** binary
   (`cargo`, `node`, `pnpm`, …): retry **once** with `nix develop -c …`, or one
   outer `mise exec` / `mise run` when `mise.toml` pins that tool.
4. Batch toolchain steps in **one** develop/mise wrap — never a fresh develop
   per small command.

```bash
# Prefer
bash scripts/verify.sh
cargo test -p cdamux-cli

# Last resort
nix develop -c bash scripts/verify.sh
```
