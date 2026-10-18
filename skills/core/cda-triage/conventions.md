# cda-triage conventions

Resolve `OWNER/REPO`, `PROJECT_ID`, and the Status field/option IDs with
`cda-kanban`'s `scripts/gh-board.sh env` — never hardcode them.

## Role → label / board Status

| Role | Expressed as | Board Status |
| ---- | ------------ | ------------ |
| `bug` (category) | label `type:fix` | — |
| `enhancement` (category) | label `type:feat` | — |
| `needs-triage` | no state label, no type yet | Backlog |
| `needs-info` | label `triage:needs-info` | Backlog |
| `ready-for-agent` | agent brief comment attached | **Ready** |
| `ready-for-human` | label `triage:ready-for-human` | **Ready** |
| `wontfix` | — | issue **closed** |

Every triaged item carries exactly one category label and one state. `ready-for-agent`
is signalled by the brief + Status Ready (no extra label needed), which is exactly
what `cda-dev --mode ready-pickup` discovers. `ready-for-human` sits in Ready too but
is flagged by its label so an agent doesn't grab it.

## Transitions

Unlabeled → `needs-triage` → (`needs-info` | `ready-for-agent` | `ready-for-human` |
`wontfix`). `needs-info` returns to `needs-triage` when the reporter replies. The
maintainer may override at any point; flag unusual transitions and ask first.

## gh recipes

```bash
eval "$(bash ~/.cursor/skills/cda-kanban/scripts/gh-board.sh env)"   # OWNER/REPO/PROJECT_ID/…

# buckets for "what needs attention"
gh issue list --repo "$OWNER/$REPO" --search "no:label" --state open      # unlabeled
gh issue list --repo "$OWNER/$REPO" --label triage:needs-info --state open

# external PRs only (discovery): exclude collaborators via author association
gh pr list --repo "$OWNER/$REPO" --state open \
  --json number,title,author,authorAssociation \
  -q '.[] | select(.authorAssociation != "MEMBER" and .authorAssociation != "OWNER" and .authorAssociation != "COLLABORATOR")'

# apply an outcome
gh issue edit  <n> --repo "$OWNER/$REPO" --add-label type:fix
gh issue comment <n> --repo "$OWNER/$REPO" --body-file brief.md   # brief starts with the AI disclaimer
gh issue close <n> --repo "$OWNER/$REPO" --comment "…"            # wontfix
# board Status moves: use cda-kanban (Ready / Backlog) — it owns the field/option IDs
```

## Wiring

- **Verify before briefing.** A reproduced bug / checked-out PR makes a stronger
  `ready-for-agent` brief (step 3).
- **Dedup against `cda-foam`.** Read `foam/features/out-of-scope.md` before
  recommending; a rejected enhancement is appended there on `wontfix` (cda-foam B2).
- **Brief ⇒ `cda-dev`.** `ready-for-agent` + Status Ready is the handoff: the brief
  is the behavioral contract `cda-dev`'s worker implements. Keep it path-free and
  acceptance-driven (see SKILL.md, "durable, behavioral contracts").
  Without `plans/PLAN-<id>-*`, orch runs **fast-lane** (`LANE=fast`); with a plan,
  **full**. Fuzzy enhancements still get PRD + plan before Ready (ADR-002).
- **Grill via `cda-grill`**; record domain/ADR updates through `cda-foam`.
