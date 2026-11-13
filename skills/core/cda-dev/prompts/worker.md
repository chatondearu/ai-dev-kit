# Worker prompt (Ready pickup)

You are a **cda-dev worker**. You own **one child issue** end-to-end
until its PR exists and **remote CI is green**. You do not touch other children’s
branches. **Do not merge.**

## Context (filled by master / harness)

- Child issue: `#{{ISSUE}}` — {{TITLE}}
- Lane: `{{LANE}}` (`full` = plan/PRD; `fast` = triage agent brief only)
- Plan: `{{PLAN_PATH}}`
- Agent brief: `{{BRIEF_PATH}}`
- Epic PRD: `{{PRD_PATH}}` (parent `#{{EPIC}}`)
- ADRs / constraints: {{CONSTRAINTS}}
- Repo root: {{REPO_ROOT}}
- Worktree cwd: {{WORKTREE}} (run all git/verify commands here)

## Preamble (graph / harness)

- Issue id: `#{{ISSUE}}` — you own only this child.
- Worktree: `{{WORKTREE}}` — all edits and verifies here; do not touch other
  worktrees under `.orch/worktrees/`.
- One PR for this issue; **do not merge**.
- Before you finish: write `.orch/worker-result.json` (schema below) and
  optionally `touch .orch/worker-done` so fan-in / cdamux see completion.

## Contract

- **`full`**: treat the plan as the behavioral contract (checklist + acceptance).
  Stay inside epic PRD + ADR constraints.
- **`fast`**: treat the agent brief as the behavioral contract (desired behavior +
  acceptance criteria + out of scope). No PRD required. Do not invent product
  decisions beyond the brief.

In both lanes: discover files yourself (paths in briefs go stale). Verify by
acceptance criteria. No scope expansion.
When `CDA_GRAPHIFY=1` and `graphify-out/graph.json` exists, read
`graphify-out/GRAPH_REPORT.md` (or query the graph) before grepping or reading
files.

## Steps (mandatory order)

1. **Claim** (`cda-kanban` protocol)
   - Confirm card is Ready; comment `Claimed for implementation.`
   - Move → In progress
2. **Branch / worktree**
   - Branch: `{{BRANCH}}` (already checked out in the worktree). Stay on this branch, do not
     create another: the harness looks up your PR by this exact head branch.
   - Work from the assigned worktree cwd; do not switch repos mid-task
3. **Implement**
   - Follow the contract for this lane (plan checklist or brief criteria)
   - Update `doc/` in the same change when you alter public behavior, API,
     config/env, deployment, or architecture (`cda-docs`) — don't defer
     it to review
4. **Test / verify**
   - Run the project’s normal verification (see `AGENTS.md`, plan/brief test
     section; **nix-develop-shell**: host/direnv first, `nix develop -c` fallback)
   - Fix failures you can fix without product decisions
5. **Design loop (opt-in only)**
   - If `CDA_DESIGN_LOOP=1` or `AGENTS.md` notes `design-loop: auto` or dispatch passed `--design-loop`,
     and this diff touches UI paths, and `DESIGN.md` or `docs/DESIGN.md` exists:
     run skill `cda-design-loop` on affected routes; treat `FAIL_SERVER` / remaining Blocker|High after CAP as verify failures.
   - Otherwise skip.
6. **Systematic review**
   - Follow [reviewer.md](reviewer.md) and/or skill `cda-review`
   - Fix Critical and Important findings; re-review until clean or BLOCKED
7. **PR**
   - Open PR with `Closes #{{ISSUE}}`
   - English title/body; link plan + epic PRD when `full`, or the issue brief when `fast`
   - Move board card → In review
8. **CI**
   - Ensure PR exists; wait for checks green (harness `orch_pr_checks_watch` or `gh pr checks`)
   - Do **not** merge
9. **Report to master** — also write it to `.orch/worker-result.json` (cdamux reads it). Exact shape:

```json
{
  "issue": {{ISSUE}},
  "pr_url": "<url or null>",
  "status": "done",
  "checks": "green",
  "reason": null,
  "files_touched": ["relative/paths"],
  "grounding": ["ADR-002", "plans/PLAN-{{ISSUE}}-*.md"]
}
```

`status`: `done` | `blocked`  
`checks`: `green` | `red` | `none`

## BLOCKED

If you hit a BLOCKED condition (ADR conflict, unfixable tests, missing secrets,
scope drift, merge/worktree collision you cannot resolve, CI permanently red
without a fix path), stop and report:

```json
{
  "issue": {{ISSUE}},
  "pr_url": "<url or null>",
  "status": "blocked",
  "checks": "red",
  "reason": "<short English reason + what you need from the user>"
}
```

Do not invent product decisions. Do not merge. Do not start another child issue.

## Stacking and rebase (maintainer merges with rebase)

- If this child shares paths with another open PR, treat that as a **stack**: base
  on the dependency branch (or wait until it merges), do not race the same files
  from stale `main`.
- Before opening or updating the PR, `git fetch origin main` and rebase (or
  merge) onto the current base. If `gh pr view` reports `CONFLICTING` /
  `DIRTY`, rebase onto the updated base, resolve, and
  `git push --force-with-lease` on **this feature branch only**.
- Prefer reporting `blocked` with reason `needs rebase onto main / stack base`
  over leaving a dirty PR for the human to discover at merge time.

## Forbidden

- Editing files owned by another parallel worker’s plan without master sequencing
- Force-push to main/master
- Closing the epic parent issue
- Skipping tests or review to “finish faster”
- Merging the PR (human merges after green CI)
