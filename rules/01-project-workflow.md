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
