# Global instructions (Claude Code)

Auto-generated from `rules/*.md` in **ai-dev-kit**. Do not edit this assembled
file by hand — change files under `rules/` and run:

```bash
./scripts/assemble-claude-md.sh
./install.sh --claude
```

Portable skills are symlinked into `~/.claude/skills/`. Per-repository guidance
lives in each project's `AGENTS.md` (bootstrap with skill `project-agents-setup`).

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

1. **Bootstrap** if missing: run `project-agents-setup/scripts/setup-agents.sh`
   (and `--init-foam` when `foam/` does not exist).
2. **Before** planning or implementing a feature: read `AGENTS.md`, then
   `foam/index.md`; check Kanban issues and open PRs for overlap.
3. **Before** any technical, product, or design decision that affects the
   project: search existing ADRs, foam notes, GitHub issues, and PRs; read
   relevant material and **confront** the new proposal with recorded decisions.
4. **During** work: keep PRD/plan/ADR artifacts aligned (`foam-project-memory`,
   `github-kanban-orchestrator`).
5. **After** shipping or deciding: update foam (ADR, PRD status, feature lists)
   and `AGENTS.md` when stack, skills, or protocols change.

Skills (ai-dev-kit): `foam-project-memory`, `foam-prd`, `foam-plan`,
`dev-orchestrator` (`foam-batch-orchestrator` alias), `github-kanban-orchestrator`,
`project-agents-setup`, `git-commit`, `code-reviewer`.

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
