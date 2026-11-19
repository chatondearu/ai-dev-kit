# PLAN-408 — chore(kit): roll out foam/board to non-Beamy cdamux projects

- **Status**: draft
- **PRD**: [[../prd/PRD-403-foam-first-kanban|PRD-403]]
- **GitHub**: [#408](https://github.com/chatondearu/ai-dev-kit/issues/408)
- **Date**: 2026-11-19

## Summary

After the kit dogfoods `foam/board` and skills list-ready from it (#405),
scaffold the same board in every non-Beamy cdamux project discovered at
runtime: one-shot map of existing GH issues/Projects, then stop writing
Projects. Confirm remotes; skip Beamy by org/remote. Do not open GitHub
issues on those repos. Do not commit the runtime project set into this kit.

Blocked by: #405.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]] | Kit first; skip Beamy (`beamysoftware/*` and remotes identified as Beamy at runtime); no Projects SoT; no GH issues for the rollout itself; no host inventory in git |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] | Project set from `cdamux projects` (runtime, not this file) |
| [[../foam/product/documentation-layout|documentation-layout]] | `foam/board/` only; no parallel kanban trees |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Runtime set = `cdamux projects` minus Beamy (org/remote pattern)
- Per remaining repo: `foam/board/` README + template (from kit), map open
  issues to cards once if the repo already uses GH issues, stop Projects writes
- Record locally (`$CDAMUX_HOME`) whether each included project got a board
  or a skip reason — not in this plan

### Out of scope

- Implementing #405 / #407
- Opening issues on target repos
- Committing host paths or other-project inventories into this public kit
- Visiting Beamy remotes

## Technical approach

Kit is already dogfooded by #404. Use the #405 import/map helper if it
exists; otherwise copy README + `_template.md` and create cards from
`gh issue list --state open` into `foam/board/{N}-{slug}.md` with
`github_issue: N`. One commit per repo in that repo's own tree. This kit
issue tracks kit-side mechanism only.

## Tasks

- [ ] Skip Beamy by org/remote (`beamysoftware/*` and remotes identified as Beamy at runtime)
- [ ] Each included project at runtime got a board or a skip reason, recorded locally
- [ ] No `gh issue create` on those repos for this rollout
- [ ] Kit-side tracking PR `Closes #408` (no host inventory in the PR)

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Distinct remotes sharing a name | Confirm `git remote -v`; skip `beamysoftware/*` |
| Repo without foam scaffold | Add `foam/board` only; do not invent a full foam graph unless already present |
| Mapping creates noisy cards | Map open issues only; skip closed |
| Host inventory leaks into the public kit | Runtime discovery stays on the operator machine |

## Test plan

Seams: skip filter vs runtime include set; kit `foam/board` already valid from #404.

- [ ] Kit board still glob-readable
- [ ] Each included project at runtime has `foam/board/` or a skip reason recorded locally
- [ ] `bash scripts/verify.sh` on the kit tracking PR

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR if a new decision was made
