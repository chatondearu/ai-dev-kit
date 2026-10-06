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
