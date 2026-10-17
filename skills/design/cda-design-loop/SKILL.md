---
name: cda-design-loop
description: >-
  Closed visual design loop for rendered UI: reach the running app, screenshot
  key routes at 375/768/1440 when eyes exist, score against DESIGN.md, fix
  Blocker/High findings up to 3 times. Use after building or redesigning pages,
  on "design pass", "looks wrong", "review the page visually", or when a web
  worker finishes UI work and a project DESIGN.md exists.
---

# CDA design loop (visual)

Announce: **Using cda-design-loop.**

## Inputs (resolve in order)

1. **Brief:** Read `DESIGN.md`, else `docs/DESIGN.md`. If absent → seed a **taste stub** (or ask once for vibe), not a brand pack. Copy from [templates/DESIGN.stub.md](templates/DESIGN.stub.md) or run the project seed script when available. Use **`cda-design-md`** only when a brand is already evidenced on the project (existing brief names Linear/Notion/Cal/Raycast/Cursor/Resend, a copied brand file, or repo already ships that language) or the user explicitly asks for a curated brand.
2. **Base URL:** `DEV_URL` env, else ask once, else probe common defaults (`http://127.0.0.1:3000`, `http://127.0.0.1:5173`) with a single HTTP check. If the probe fails, run the project's documented dev start command once (`AGENTS.md`, `package.json` — e.g. `pnpm dev`, `npm run dev`), wait briefly, re-probe; only then **FAIL_SERVER**.
3. **Routes:** user args, else UI routes touched in the diff, else `/`.
4. **Eyes backend:** first available among:
   - Playwright MCP / browser automation already in the session
   - Cursor / host browser tooling that can screenshot
   - Target repo helper `scripts/cda-design-shot.sh` (when scaffold copied it there)
   - Else **SKIP visual** — still run textual checklist + HTTP reachability

## Brand rule

Curated brand templates are **not** the default for greenfield work. Apply `cda-design-md` only when evidenced, named in an existing brief, or the user requests a brand. Otherwise use the taste stub at `templates/DESIGN.stub.md` (via seed or manual copy).

## Loop protocol (cap 3)

```
for i in 1..3:
  assert HTTP 200 on each route (start dev server once per Inputs if probe failed; fail if still dead — never count as SKIP pass)
  if eyes available:
    screenshot each route @ 375, 768, 1440
  run cheap gates:
    - console errors / failed network (if eyes can observe)
    - landmark / label smell (a11y tree or DOM skim)
    - anti-slop deterministic checks (see reference.md)
  critique screenshots against DESIGN.md (agent vision / image read)
  triage findings: Blocker | High | Medium | Nitpick
  if no Blocker and no High: exit success
  apply fixes for Blocker + High only
emit leftover Medium/Nitpick for the human
```

## Exit criteria

| Result | Meaning |
| ------ | ------- |
| **PASS** | No Blocker/High left; screenshots or SKIP note attached to reply |
| **PASS_WITH_NITS** | Only Medium/Nitpick remain after ≤3 loops |
| **FAIL_SERVER** | Dev server unreachable |
| **SKIP_VISUAL** | Eyes unavailable; textual/audit pass only — state this explicitly |
| **CAP** | Still Blocker/High after 3 loops — stop and list remaining |

**Never claim visual PASS when eyes were SKIP.** A dead server is **FAIL_SERVER**, not SKIP.

## Rubric

Full Must / Should / Anti-slop checklist and severity rules: [reference.md](reference.md).

## Sibling design skills

| Skill | Role relative to loop |
| ----- | --------------------- |
| `cda-design-md` | Seed / refresh `DESIGN.md` before loop |
| `cda-design-taste` | Direction when no brand template |
| `cda-design-bold` | Creative build without brand; loop still needs a short brief |
| `cda-design-audit` | Checklist pass inside or after loop (Vercel Web Interface Guidelines) |
| `cda-prototype` | **Before** commit-to-one-design; not inside the loop |
