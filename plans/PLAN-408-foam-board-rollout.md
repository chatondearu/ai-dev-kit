# PLAN-408 — chore(kit): roll out foam/board to non-Beamy cdamux projects

- **Status**: draft
- **PRD**: [[../prd/PRD-403-foam-first-kanban|PRD-403]]
- **GitHub**: [#408](https://github.com/chatondearu/ai-dev-kit/issues/408)
- **Date**: 2026-11-19

## Summary

After the kit dogfoods `foam/board` and skills list-ready from it (#405),
scaffold the same board in every non-Beamy cdamux project: one-shot map of
existing GH issues/Projects, then stop writing Projects. Confirm remotes;
skip Beamy. Do not open GitHub issues on those repos.

Blocked by: #405.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-016-foam-first-kanban|ADR-016]] | Kit first; skip Beamy / `beamysoftware/*`; no Projects SoT; no GH issues for the rollout itself |
| [[../foam/decisions/ADR-015-global-and-nested-orchestrators|ADR-015]] | Project set from `cdamux projects` |
| [[../foam/product/documentation-layout|documentation-layout]] | `foam/board/` only; no parallel kanban trees |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Confirm each include-list remote; skip if `beamysoftware/*` or not git
- Per remaining repo: `foam/board/` README + template (from kit), map open
  issues to cards once if the repo already uses GH issues, stop Projects writes
- Tick the checklist on this issue (kit tracker)

### Out of scope

- Implementing #405 / #407
- Opening issues on target repos
- Beamy skip list (do not visit those trees)

## Technical approach

Kit is already dogfooded by #404. Use the #405 import/map helper if it
exists; otherwise copy README + `_template.md` and create cards from
`gh issue list --state open` into `foam/board/{N}-{slug}.md` with
`github_issue: N`. One commit per repo in that repo's own tree. This kit
issue only tracks the checklist.

## Tasks

- [ ] Skip list honoured: `beamy-v2`, `data-pipelines`, `dev-setup`, `web-extension`, `windows-agent`, `/hdd/dev/chatondearu/beamy`, any `beamysoftware/*` remote
- [ ] For each include below: remote confirmed or skipped with reason; `foam/board` present; existing open GH issues mapped once **or** explicitly left unmapped if the repo has no issues; Projects no longer in the default agent path
- [ ] No `gh issue create` on those repos for this rollout
- [ ] This issue checklist ticked; PR `Closes #408` on the kit (tracking only)

### Include checklist (confirm remotes)

- [ ] bajocraft
- [ ] bajometer
- [ ] barajeuxonline
- [ ] cda (both paths if distinct remotes)
- [ ] ai-dev-kit (kit dogfood — already from #404)
- [ ] ai-dev-kit-reconcile
- [ ] cool-tts-service
- [ ] dotfiles
- [ ] eslint-config
- [ ] fluffmind
- [ ] foam
- [ ] glitchmao
- [ ] melotts-coolify
- [ ] mirabelle-ha-blueprints
- [ ] mirabelle-ha-blueprints-cda-alarm-pr
- [ ] my-cooli-claw
- [ ] myrabelle-ha-config
- [ ] neomind
- [ ] nomorecallcenter
- [ ] uno-variations
- [ ] vue-maplibre
- [ ] wiregraph
- [ ] motoko-ops
- [ ] nixos

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Distinct remotes sharing a name | Confirm `git remote -v`; skip `beamysoftware/*`; note both `cda` paths |
| Repo without foam scaffold | Add `foam/board` only; do not invent a full foam graph unless already present |
| Mapping creates noisy cards | Map open issues only; skip closed |

## Test plan

Seams: skip filter vs include list; kit `foam/board` already valid from #404.

- [ ] Kit board still glob-readable
- [ ] Each visited repo either has `foam/board/` or a skip reason on this issue
- [ ] `bash scripts/verify.sh` on the kit tracking PR

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR if a new decision was made
