# PLAN-407 — feat(cdamux): global foam-board resume index under CDAMUX_HOME

- **Status**: draft
- **PRD**: [[../prd/PRD-403-foam-first-kanban|PRD-403]]
- **GitHub**: [#407](https://github.com/chatondearu/ai-dev-kit/issues/407)
- **Date**: 2026-11-19

## Summary

Write `$CDAMUX_HOME/board-index.json` as a cache of Ready / in-progress /
blockers / open PRs aggregated from each non-Beamy project's `foam/board`.
Refresh is a command the global orch can run at turn start and on an
interval. Git is the sync; the file is never SoT.

Blocked by: #404 (card contract). Parallel with #405 (crates vs skills).

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]] | Path `$CDAMUX_HOME/board-index.json`; skip Beamy; missing foam = report, no crash |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | State root, `cat`/`jq` readable, atomic write (tmp + rename); no second store |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] | Discovery matches `cdamux projects`; global orch is the primary reader |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local filesystem only |
| `cdamux/AGENTS.md` | Tests on throwaway tmux/state dirs only; never live `$CDAMUX_HOME` |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Refresh: for each included project, fetch/check foam (git), parse cards,
  optionally note open PRs (`gh pr list` when a card has `github_issue` or
  the repo is git+gh), write the index atomically
- Read: `cat` the file; optional `cdamux board` / `cdamux board refresh`
  wrappers that do not change the schema
- Skip list: names/paths in ADR-016 plus any `beamysoftware/*` remote
- Missing foam/board: entry with an error/skip field; continue

### Out of scope

- `cda-orchestrator` prose to read the file (#405)
- Scaffolding `foam/board` in other repos (#408)
- Pushing status into the index as SoT
- Cross-machine transport (git pull is enough)

## Technical approach

Reuse project discovery from `cdamux projects`. Filter skip list before
any git work. Per project: if `.git` and `foam/board/` exist, glob `*.md`
except README/`_template.md`, parse YAML frontmatter; else record skip.
Open PRs are additive (best-effort `gh`); a `gh` failure must not abort
the rest. Write JSON with a `generated_at` and `projects: []`. Schema
stable enough for `jq '.projects[] | select(.ready|length>0)'`.

## Tasks

- [ ] Refresh writes `$CDAMUX_HOME/board-index.json` atomically; a second refresh with no card changes is idempotent aside from `generated_at`
- [ ] Included project with foam/board: Ready, in-progress, in-review, `blocked_by`, and known open PRs appear
- [ ] Beamy names/paths and `beamysoftware/*` remotes are absent
- [ ] Missing foam or foam/board: project entry with skip/error, exit 0
- [ ] CLI or documented `cat` path so the global orch reads it in one shot; refresh invoked without a TUI
- [ ] Tests use a temp `CDAMUX_HOME` and temp repos; `cargo test` + `bash scripts/verify.sh`; PR `Closes #407`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| `git pull` mutates operator trees | Prefer `git fetch` + read `foam/board` at the commit you already have; pull only if a flag/docs say so — default: read local worktree, report if board missing |
| `gh pr list` rate limits | Best-effort; index stays valid without PRs |
| Schema churn vs #405 | Keep ADR-016 path and top-level keys (`generated_at`, `projects`) stable |

## Test plan

Seams: skip filter; parser; refresh over a fake projects list; JSON file
shape. No live tmux required for the parser/skip tests.

- [ ] Skip filter unit tests
- [ ] Temp repo with/without foam; index contains skip vs cards
- [ ] Throwaway `CDAMUX_HOME` only
- [ ] `bash scripts/verify.sh`

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR if a new decision was made
