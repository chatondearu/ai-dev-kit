---
name: cda-design-md
description: >-
  Apply a curated brand DESIGN.md from the kit when the user wants UI that
  matches Linear, Notion, Cal, Raycast, Cursor, or Resend. Use when dropping a
  design-system brief into a project, matching an existing product look, or
  picking a starting DESIGN.md before coding UI.
---

# CDA Design.md (brand templates)

Announce: **"Using cda-design-md skill."**

## Role

Provide a **ready-to-copy brand design brief** (`DESIGN.md`) so agents generate
UI consistent with a known product language. Templates are a curated subset of
[awesome-design-md](https://github.com/VoltAgent/awesome-design-md).

## When to use which design skill

| Skill | Use when |
| ----- | -------- |
| **cda-design-md** | Match one of the curated brands below |
| **design-taste-frontend** | Anti-slop direction for landings / portfolios / redesigns |
| **frontend-design** | Bold creative UI without a brand template |
| **web-design-guidelines** | Audit interactions, a11y, UX quality after UI exists |

## Brands shipped

| Brand | Path |
| ----- | ---- |
| Linear | [brands/linear.app/DESIGN.md](brands/linear.app/DESIGN.md) |
| Notion | [brands/notion/DESIGN.md](brands/notion/DESIGN.md) |
| Cal | [brands/cal/DESIGN.md](brands/cal/DESIGN.md) |
| Raycast | [brands/raycast/DESIGN.md](brands/raycast/DESIGN.md) |
| Cursor | [brands/cursor/DESIGN.md](brands/cursor/DESIGN.md) |
| Resend | [brands/resend/DESIGN.md](brands/resend/DESIGN.md) |

## Workflow

1. Ask which brand (or infer from the product brief).
2. Read the matching `brands/<name>/DESIGN.md`.
3. Copy it into the target repo as `DESIGN.md` (or `docs/DESIGN.md`) when the
   user wants a durable project brief.
4. Implement UI following that file; do not invent a conflicting system.
5. Optionally run **web-design-guidelines** for a compliance pass.

## Extending the set

Upstream catalogue: https://github.com/VoltAgent/awesome-design-md  
Add new brands under `brands/` and list them in this skill + `THIRD_PARTY.md`.
