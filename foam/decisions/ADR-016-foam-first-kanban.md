# ADR-016 — Foam-first kanban, optional GitHub issues, worker meta auto-merge

- **Status**: proposed
- **Date**: 2026-11-19
- **Tags**: #architecture #tooling #autonomy #foam
- **GitHub**: [#404](https://github.com/chatondearu/ai-dev-kit/issues/404)
  (epic [#403](https://github.com/chatondearu/ai-dev-kit/issues/403))

## Context

Execution status today lives on **GitHub Projects** (`cda-kanban` `gh-board.sh`,
`orch-auto` `list-ready`). That costs tokens and round-trips, conflicts with
foam as agent memory, and makes a global orchestrator (ADR-015) poll every
repo's Projects API to resume work.

The maintainer decided (2026-11-19, global orch `a-d6bz3`):

1. Project status lives in **foam**, not GitHub Projects.
2. GitHub Issues are **optional** (open one when a ticket needs GH discussion,
   `Closes`, or an external tracker).
3. Workers **self-merge** board/meta PRs after the usual local gate. No GitHub
   ruleset or automerge app.
4. A machine-local **resume index** under `$CDAMUX_HOME` (ADR-011) is a cache
   rebuilt from each project's foam board. Git is the sync.

Fleet already browses `foam/` (epic [#265](https://github.com/chatondearu/ai-dev-kit/issues/265),
[[ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]]). This ADR is the
**board SoT + orch**, not that UI. Do not reopen #265.

[[ADR-002-orch-auto-and-fast-lane|ADR-002]] still owns fast-lane vs full-lane
and the orch-auto poller. This ADR changes **where Ready is listed** and
**narrows** ADR-002's "no auto-merge" constraint to product/`doc/` diffs.

## Decision

### 1. Foam-first board

- Source of truth for card status is **`foam/board/`**, versioned in git.
- **One markdown file per card** (avoids worker merge conflicts on a shared
  board file). Readers glob `foam/board/*.md` and **skip** `README.md` and
  names starting with `_` (template). There is **no** hand-edited aggregate
  index in the repo. A file without required frontmatter is not a card.
- Status flow is unchanged: Backlog → Ready → In progress → In review → Done.
  Do not invent a second kanban.
- YAML frontmatter (required: `status`, `priority`, `updated`; optional:
  `github_issue`, `blocked_by`, `title`):

  ```yaml
  ---
  title: "feat(scope): imperative headline"
  status: backlog   # backlog | ready | in-progress | in-review | done
  priority: P1      # P0 | P1 | P2
  updated: 2026-11-19
  github_issue: 405   # omit when the card has no GitHub issue
  blocked_by: [404]   # issue numbers and/or card slugs; omit if none
  ---
  ```

- Filename: `{slug}.md`. When `github_issue` is set, prefer `{N}-{slug}.md`.
  Workers rename a card only when they own it.
- GitHub Issues are **opt-in** via `github_issue`. `gh issue` /
  `Closes #N` only for those cards. A card with no `github_issue` is still
  pickable from foam.

### 2. GitHub Projects is not SoT

- Default: **no Projects API** in list-ready, orch-auto, or status moves.
- An optional Projects **mirror** is allowed only if it is measured cheaper
  in tokens/time than foam-only. Until that measurement exists, do not
  write Projects.
- `gh-board.sh` remains for cards that opted into a GitHub issue (comments,
  `Closes`, optional board mirror). It is not the Ready oracle.

### 3. Worker self-merge of meta PRs

After verify **and** the ADR-006 independent review gate, a worker **merges
its own PR** (`gh pr merge`) when **every** changed path is in:

| Allow | Path |
| ----- | ---- |
| yes | `foam/board/**` |
| yes | `prd/` |
| yes | `plans/` |
| yes | `foam/decisions/` |
| yes | other `foam/**` that does not change product status (MOC, architecture notes, board README) |
| no | `doc/`, `README*`, `skills/`, `cdamux/` crates, application code, `install.sh`, Nix modules |

**Fail closed**: any path outside the allowlist → do not merge; report for
maintainer merge. No GitHub ruleset, merge queue, or automerge app.

This **narrows** ADR-002's "no auto-merge" and the cda-dev hard rule to
product/`doc/` diffs. Review-gate (ADR-006) is unchanged.

### 3b. Board reconcile (amendment 2026-11-23, owner decision Q10)

Nobody is left to move a card to `done` after a merge (the worker is gone; a
maintainer merge runs nothing). `foam-board.sh reconcile --pr` closes that gap:

- **Drift**: a card `in-progress` or `in-review` with a PR merged into the
  default branch on or after the card's `updated` day (its claim / in-review
  move), whose head is `feat/<id>-issue-<id>` or which closes the card's
  `github_issue` **in this repository**. It moves to `done`.
- **Never on the default branch**: a throwaway worktree off a fresh
  `origin/<default>`, branch `board/reconcile-<stamp>`, one foam-only PR.
- **Self-merge**: through `orch_merge_pr` (rebase merge, merge lock,
  `--match-head-commit`), ADR-016 allowlist, checks green or allowed-none.
  No agent review and no repo verify: the script refuses to push unless the
  staged diff is only `status:`/`updated:` lines of existing `foam/board/`
  cards.
- **Issues**: a card's `github_issue`, if still open, is closed only after
  the board PR merged. Cards without `github_issue` touch no issue.
- **No waiting**: one try to land, no CI watch, so no run lock or pool slot
  is held on it. A PR left open (pending checks, merge lock busy) is landed
  by the next reconcile; one behind the default branch is closed and
  regenerated; red checks leave it for the maintainer.
- **Callers**: orch.sh after a successful ADR-016 merge and at the end of
  every run (`ORCH_BOARD_RECONCILE=0` turns it off). One reconcile per repo
  (mkdir lock in the git common dir; dead pid, or no pid after a minute, is
  stale); the open-PR check runs under that lock, so no duplicate PRs.
- **`cdamux board refresh` stays read-only**: it reports drift in the index
  (`drift`) and on stderr, never commits or opens a PR. Refresh runs on every
  orchestrator turn and across many projects; writes there would race the
  orch.sh reconcile and open PRs on repos nobody is working in.

### 4. Global resume index (cache, not SoT)

- Path: **`$CDAMUX_HOME/board-index.json`** (ADR-011 state root). Readable
  with `cat` / `jq`. Optional `cdamux board` CLI is a wrapper, not a second store.
- Contents: per included project, Ready cards, in-progress, blockers
  (`blocked_by` or worker `blocked`), and open PRs the refresh can see.
- **Rebuild from foam**: refresh pulls/checks each project's `foam/board`
  (git is the sync across machines). The index is never authoritative.
- Refresh: a command the global orch runs at **turn start** and on a
  **frequent interval**. Idempotent.
- Project set: `cdamux projects`, **minus Beamy** (`beamysoftware/*`
  remotes and remotes identified as Beamy at runtime). Do not commit the
  runtime project list or host filesystem paths into this repository.
- A repo missing `foam/` or `foam/board/` is **reported in the index** and
  skipped; refresh does not crash.

### 5. Rollout

Kit dogfood first, then every cdamux project that is not Beamy. Per repo:
scaffold `foam/board`, one-shot map of existing GH issues/Projects, then
stop writing Projects. Do not open GitHub issues on those repos for the
rollout itself.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Keep GitHub Projects as SoT, foam as a mirror | Token/time cost of Projects API; foam is already agent memory |
| One `foam/board.md` / generated index in-repo | Worker merge conflicts (owner constraint: 1 card = 1 file) |
| Require a GitHub issue per card | Optional by decision; foam-only cards must be pickable |
| GitHub ruleset / automerge app for meta paths | Owner: workers `gh pr merge` themselves after the local gate |
| Auto-merge `doc/` / README / skills | User-facing and product; fail closed until the owner expands the list |
| Index as a second SoT (push status into `$CDAMUX_HOME` only) | Same repo on several machines; git is the sync |
| Include Beamy in the index / rollout | Owner skip: `beamysoftware/*` and remotes identified as Beamy at runtime |

## Consequences

- **Positive**: orch-auto and the global orch resume from git-versioned
  cards; meta PRs land without a maintainer click; #265 Foam browser already
  shows `foam/board/` once files exist.
- **Negative**: agents must `git pull` before listing Ready; a stale
  worktree can pick a card that moved. The index can lag until refresh.
- **Constraint**: do not call Projects in the default Ready path; do not
  auto-merge outside the allowlist; do not treat `$CDAMUX_HOME/board-index.json`
  as authoritative.
- **Amends** [[ADR-002-orch-auto-and-fast-lane|ADR-002]]: Ready listing from
  foam; meta-path auto-merge allowed. Fast-lane / poller / systemd templates
  unchanged.
- **Does not change** [[ADR-006-review-gate-fail-closed-verify|ADR-006]],
  [[ADR-011-cdamux-unified-state-model|ADR-011]] layout (index is one more
  file under the state root), [[ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]],
  [[ADR-015-global-and-nested-orchestrators|ADR-015]].

## Amendments

- **2026-11-23** (orchestration audit C2/H1, owner decisions Q4/Q5/Q26): the
  self-merge is `gh pr merge --rebase --match-head-commit` — never the repo
  default method, which `gh` cannot pick non-interactively. Before merging,
  `orch.sh` takes the per-repo `.orch/merge.lock` (`flock(2)`; manual merges via
  `scripts/merge-lock.sh merge <N>`, same checks), checks the PR head is the feature branch, fetches `origin/<default>`
  (resolved, never assumed `main`), rebases + re-runs verify + pushes the
  feature branch when it is behind, and refuses `mergeable=UNKNOWN`. A repo
  that disallows rebase merges, or any failed step, is an `error` (no fallback
  method, never reported `done`). The owner enables branch protection
  "require branches to be up to date" on this repo. The same pre-merge rule
  is written into skill `cda-orchestrator` for the maintainer's merges.

## References

- Epic [#403](https://github.com/chatondearu/ai-dev-kit/issues/403); children
  #404 (this ADR), #405 (skills), #406 (auto-merge), #407 (index), #408 (rollout)
- Foam browser overlap: [#265](https://github.com/chatondearu/ai-dev-kit/issues/265)
  — do not reopen
- Card contract: [[../board/README|foam/board/README]]
- [[../product/documentation-layout|Documentation layout]]
- PRD: [[../../prd/PRD-403-foam-first-kanban|PRD-403]]
