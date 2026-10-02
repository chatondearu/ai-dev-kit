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
