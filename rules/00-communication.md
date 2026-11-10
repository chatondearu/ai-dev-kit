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
