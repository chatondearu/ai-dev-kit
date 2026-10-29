# Design: `cda-design-loop` + web design scaffold

Date: 2026-10-17  
Status: approved (2026-10-17)

## Goals

1. Give agents a **closed visual loop** after UI work: see the rendered page, score it against a project brief, fix, stop.
2. Make that loop **discoverable** next to existing `cda-design-*` skills and usable without a giant harness.
3. Seed **new/existing web projects** with a durable design brief (`DESIGN.md` + optional tokens) so generation starts from direction, not from the training-data mean.

## Non-goals

- Vendoring Playwright, a full UI harness, Superdesign, or ECC.
- Auto-installing browser tooling into consumer repos.
- Replacing `cda-design-taste` / `cda-design-bold` / `cda-design-audit` — the loop **orchestrates** them.
- Vision API calls as a hard dependency (agent-reads-PNG is enough when the host can show images).

## Problem

Today the kit can steer generation (`cda-design-md`, taste, bold) and checklist-audit (`cda-design-audit`), plus explore variants (`cda-prototype`). What is missing:

- A **protocol** that forces “boot → look → critique → fix” with a hard iteration cap.
- A **project-local** brief so agents do not reinvent Linear-purple every chat.
- Clear **eyes** wiring (Playwright MCP / Cursor browser / local screenshot script) documented for agents.

Without eyes, “make it look good” remains hope. Without a brief, eyes only confirm generic slop.

## Decisions

| Decision | Choice | Why |
| -------- | ------ | --- |
| Shape | One skill `cda-design-loop` + thin scaffold helpers | One invoke name; scaffold is files, not a second product |
| Critic | Driving agent reads screenshots + checklist (no mandatory vision API) | Works in Cursor/Claude with image-capable models; pluggable later |
| Browser | Prefer existing host tools; optional local `scripts/cda-design-shot.sh` | No Playwright auto-install; degrade cleanly |
| Breakpoints | Fixed: `375`, `768`, `1440` | Matches common visual-review practice; enough for layout bugs |
| Iteration cap | **3** full loops, then hand residual to human | Avoid thrashing |
| Project brief path | Prefer `DESIGN.md` at repo root; else `docs/DESIGN.md` | Matches `cda-design-md` workflow |
| Tokens | Optional `design/tokens.json` (DTCG-ish primitives → semantic) | Nice for Tailwind/`@theme`; not required for loop to run |
| Hard cut | New skill name only (`cda-design-loop`); no aliases | Same as rest of kit |
| Shot script in seed | **On by default** (`scripts/cda-design-shot.sh`) | Eyes bridge without MCP; still no Playwright auto-install |
| Brand pick | Brand template **only** if already used on the project or named in existing `DESIGN.md` | Avoid forcing Linear/etc. on greenfield; else taste stub |
| Auto-loop in `cda-dev` | **Off by default** (on demand). Optional opt-in flag/env for workers | Start safe; enable later per project |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Target web repo                                        │
│  DESIGN.md (± design/tokens.json)                       │
│  AGENTS.md → points at cda-design-* + cda-design-loop   │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│  cda-design-loop (skill in ai-dev-kit)                    │
│  1. Resolve brief + routes + base URL                     │
│  2. Ensure app is reachable (or start project’s own cmd)  │
│  3. Capture eyes (MCP / browser / shot script / SKIP)     │
│  4. Score vs brief + anti-slop + light a11y               │
│  5. Fix blockers; repeat ≤ 3; then summarize leftovers    │
└───────────────────────────┬─────────────────────────────┘
                            │ uses
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
  cda-design-md/taste    cda-design-audit    host eyes
  (brief if missing)     (checklist pass)    (Playwright MCP,
                                              Cursor browser,
                                              or shot script)
```

### Skill: `cda-design-loop`

**Location:** `skills/design/cda-design-loop/SKILL.md` (+ short `reference.md` for rubric).

**When to invoke:** After UI was implemented or redesigned; when the user says “review the page visually”, “design pass”, “looks wrong”, or when a web worker finishes a UI task and `DESIGN.md` exists.

**Announce:** `Using cda-design-loop.`

#### Inputs (resolve in order)

1. **Brief:** `DESIGN.md` or `docs/DESIGN.md`. If absent → seed a **taste stub** (or ask once for vibe), not a brand pack. Apply `cda-design-md` **only** when a brand is already evidenced on the project (existing `DESIGN.md` names Linear/Notion/…, copied brand file, or repo already ships that brand language) or the user explicitly asks for a brand.
2. **Base URL:** `DEV_URL` env, else ask once, else common defaults (`http://127.0.0.1:3000`, `:5173`) with a single probe.
3. **Routes:** user args, else changed UI routes from the diff, else `/`.
4. **Eyes backend:** first available among:
   - Playwright MCP / browser automation tools already in the session
   - Cursor / host browser tooling that can screenshot
   - Kit helper `scripts/cda-design-shot.sh` if present in the **target** repo (copied by scaffold)
   - Else **SKIP visual**, still run textual checklist + HTTP reachability

#### Loop protocol

```
for i in 1..3:
  assert HTTP 200 on each route (fail if server dead — never count as SKIP pass)
  if eyes available:
    screenshot each route @ 375, 768, 1440
  run cheap gates:
    - console errors / failed network (if eyes can observe)
    - landmark / label smell (a11y tree or DOM skim)
    - anti-slop deterministic checks (see rubric)
  critique screenshots against DESIGN.md (agent vision / image read)
  triage findings: Blocker | High | Medium | Nitpick
  if no Blocker and no High: exit success
  apply fixes for Blocker + High only
emit leftover Medium/Nitpick for the human
```

#### Rubric (in `reference.md`)

Must check (Blocker/High if failed):

- Matches brief direction (tone, typography family, palette intent) — not “generic AI landing”
- Hierarchy: one clear primary action / headline per first viewport
- No overflow / overlapping text at 375 and 1440
- Focusable controls have accessible names; no unlabeled icon buttons that are sole affordance
- Console: no uncaught errors on load of reviewed routes

Should check (Medium):

- Spacing rhythm vs brief; contrast for body text
- Empty / loading / error states if those surfaces were in scope
- Motion: intentional, not noise

Anti-slop flags (deterministic when possible): Inter/Roboto/Arial as sole stack when brief forbids; purple-gradient-on-white default when brief is not that; card-grid wallpaper with no hierarchy; emoji-as-icon clusters.

#### Exit criteria

| Result | Meaning |
| ------ | ------- |
| **PASS** | No Blocker/High left; screenshots or SKIP note attached to reply |
| **PASS_WITH_NITS** | Only Medium/Nitpick remain after ≤3 loops |
| **FAIL_SERVER** | Dev server unreachable |
| **SKIP_VISUAL** | Eyes unavailable; textual/audit pass only — must say so explicitly |
| **CAP** | Still Blocker/High after 3 loops — stop and list remaining |

Never claim visual PASS when eyes were SKIP.

#### Relationship to other design skills

| Skill | Role relative to loop |
| ----- | --------------------- |
| `cda-design-md` | Seed / refresh `DESIGN.md` before loop |
| `cda-design-taste` | Direction when no brand template |
| `cda-design-bold` | Creative build without brand; loop still needs a short brief |
| `cda-design-audit` | Run once as checklist pass inside or after loop (fetch Vercel guidelines) |
| `cda-prototype` | **Before** commit-to-one-design; not inside the loop |

### Scaffold: web design seed

**Trigger:** `cda-agents` overlay `web` / `fullstack`, or a small script `skills/design/cda-design-loop/scripts/seed-design.sh` callable from agents/setup.

**Writes into the target repo (idempotent, no overwrite without `--force`):**

| Path | Content |
| ---- | ------- |
| `DESIGN.md` | Taste stub by default. Brand file from `cda-design-md` **only** if project already uses that brand or `DESIGN.md` already names it |
| `design/tokens.json` | Optional minimal DTCG stub (color/font/radius primitives → semantic) — skip if `--no-tokens` |
| `scripts/cda-design-shot.sh` | **Included by default** (disable with `--no-shot`): args `URL OUT_DIR`; uses Playwright **only if** already available — otherwise prints SKIP instructions |
| `AGENTS.md` managed block | Add rows for `cda-design-loop` and the order: brief → build → loop → audit |

**Does not:** add Playwright as a project dependency; start servers; generate UI; force a curated brand on empty projects.

### Docs / install surface

| File | Change |
| ---- | ------ |
| `skills/README.md` | List `cda-design-loop` under design |
| `AGENTS.md` + `cda-agents` templates / `overlays/web.md` | Skill table + one-line workflow |
| `cda-dev` | Document opt-in auto-loop (see below); default remains manual invoke |
| `docs/recommended.md` | “Eyes” section: Playwright MCP, Cursor browser, when to use Superdesign |
| `THIRD_PARTY.md` | Only if we vendor a tiny upstream snippet (prefer none for v1) |
| Spec pack `2026-10-07-…` | Cross-link as follow-up (optional one-liner) |

## Approaches considered

| Approach | Pros | Cons |
| -------- | ---- | ---- |
| **A. Skill-only protocol (chosen)** | Fits kit; no dep hell; works with any eyes host | Quality depends on agent discipline |
| B. Full Playwright harness in kit | Deterministic CI | Heavy; auto-install risk; overlaps consumer stacks |
| C. Vision API critic service | Consistent scores | API keys, cost, not portable |

Recommendation: **A**, with optional shot script as a bridge for agents that can shell but lack MCP.

## Error handling / degrade

- Dead server → `FAIL_SERVER`, do not SKIP.
- No eyes → `SKIP_VISUAL` + still run `cda-design-audit` if useful.
- No `DESIGN.md` → stop and offer seed / brand pick (one question).
- Shot script finds no Playwright → print install-once hint; exit SKIP for screenshots only.

## Testing (kit-side)

- Skill markdown: trigger phrases + exit criteria readable (manual / writing-for-agents pass).
- `seed-design.sh`: dry-run creates files under a temp dir; `--force` overwrite covered; no network.
- No requirement to run Playwright in kit CI for v1.

## Rollout

1. Land skill + `reference.md` + seed script.
2. Wire `cda-agents` web overlay + kit `AGENTS.md` / README / recommended.
3. Dogfood on one real web repo (screenshot loop with Cursor browser or MCP).
4. Later (out of scope v1): DTCG ↔ Tailwind sync helper; vision-API critic flag.

## Success criteria

- An agent finishing a landing change can invoke **one** skill and produce either PASS / PASS_WITH_NITS / CAP / SKIP_VISUAL / FAIL_SERVER with evidence.
- A new web project seeded via overlay has a `DESIGN.md` and knows to call the loop after UI work.
- Kit install remains free of Playwright as a hard dependency.

## Resolved (2026-10-17)

1. **Shot script:** included **by default** in seed; `--no-shot` to skip.
2. **Brand:** use a curated brand **only** if already used on the project or indicated in existing `DESIGN.md`; otherwise taste stub / explicit user request.
3. **`cda-dev` auto-loop:** **on demand** for v1. Add an **opt-in** so projects can enable auto-invoke later without changing the skill default.

### `cda-dev` opt-in (v1 surface)

When a worker’s diff touches UI paths (`*.vue`, `*.tsx`, `*.css`, routes under `pages/` / `app/`, etc.) **and** a project brief exists:

- Default: do **not** run `cda-design-loop` (human or prompt asks).
- Opt-in if any of:
  - env `CDA_DESIGN_LOOP=1` in the worker / orch environment, or
  - project file `.cda/design-loop` (or a one-liner in `AGENTS.md` Project-specific notes: `design-loop: auto`), or
  - orch / skill argument `--design-loop` when dispatching.

Document the flag in `cda-dev/SKILL.md` and the web overlay; implementation can start with env + `AGENTS.md` note only (file marker optional if env is enough for v1).

## Implementation

Plan: removed (superseded superpowers checklist). Skill: `cda-design-loop`; archive index: [[../../archive/README|foam/archive]].
