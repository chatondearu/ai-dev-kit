# `cda-design-loop` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `cda-design-loop` (visual critique protocol), a project seed script (`DESIGN.md` + shot helper + optional tokens), and wire discovery + optional `cda-dev` auto-loop opt-in — without adding Playwright as a kit dependency.

**Architecture:** Skill + `reference.md` encode the boot → eyes → rubric → fix loop (cap 3). `seed-design.sh` drops taste-stub brief and `cda-design-shot.sh` into target repos. Docs/`cda-agents` web overlay/`cda-dev` document when to invoke and how to opt into auto-loop (`CDA_DESIGN_LOOP=1`).

**Tech Stack:** Markdown skills, portable bash (seed + shot), existing design skills (`cda-design-*`), host browser/MCP (not vendored)

## Global Constraints

- Spec: `docs/superpowers/specs/2026-10-17-cda-design-loop-design.md` (approved).
- No Playwright auto-install; no vision API required; hard cut name `cda-design-loop`.
- Brand pack only if already on project or named in existing `DESIGN.md`; else taste stub.
- Shot script seeded **by default**; `--no-shot` skips it.
- `cda-dev` auto-loop **off** unless `CDA_DESIGN_LOOP=1`, `AGENTS.md` note `design-loop: auto`, or `--design-loop`.
- Breakpoints: 375 / 768 / 1440; iteration cap 3; never claim visual PASS on SKIP_VISUAL.
- English in skill bodies; Conventional Commits on commits.
- Never run `install.sh` against live config from tests — use temp `HOME` / `CURSOR_HOME` / etc.

---

## File map

| Path | Responsibility |
| ---- | -------------- |
| `skills/design/cda-design-loop/SKILL.md` | Protocol, inputs, exit codes, skill graph |
| `skills/design/cda-design-loop/reference.md` | Rubric + anti-slop flags |
| `skills/design/cda-design-loop/templates/DESIGN.stub.md` | Taste stub for greenfield seed |
| `skills/design/cda-design-loop/templates/tokens.stub.json` | Minimal DTCG-ish tokens |
| `skills/design/cda-design-loop/templates/cda-design-shot.sh` | Shot helper copied into target repos |
| `skills/design/cda-design-loop/scripts/seed-design.sh` | Idempotent seed into a target repo |
| `skills/design/cda-design-loop/scripts/tests/test_seed_design.sh` | Temp-dir assertions for seed |
| `skills/design/cda-design-md/SKILL.md` | Cross-link loop in “when to use” table |
| `skills/README.md`, kit `AGENTS.md`, `cda-agents` templates + `overlays/web.md` | Discovery |
| `docs/recommended.md` | Eyes section |
| `skills/core/cda-dev/SKILL.md`, `prompts/worker.md` | Opt-in auto-loop |

---

### Task 1: Skill + rubric + taste stub template

**Files:**
- Create: `skills/design/cda-design-loop/SKILL.md`
- Create: `skills/design/cda-design-loop/reference.md`
- Create: `skills/design/cda-design-loop/templates/DESIGN.stub.md`
- Modify: `skills/design/cda-design-md/SKILL.md` (add loop row to the comparison table)

**Interfaces:**
- Consumes: approved design spec; sibling design skills by name
- Produces: model-invoked skill `name: cda-design-loop`; exit statuses `PASS|PASS_WITH_NITS|FAIL_SERVER|SKIP_VISUAL|CAP`; brief paths `DESIGN.md` then `docs/DESIGN.md`

- [ ] **Step 1: Create directories**

```bash
mkdir -p skills/design/cda-design-loop/{scripts/tests,templates}
```

- [ ] **Step 2: Write `templates/DESIGN.stub.md`**

```markdown
# Design brief (project)

> Taste stub — replace or extend as the product language firms up.
> Do **not** paste a curated brand pack unless this project already uses that brand
> or you explicitly choose one via `cda-design-md`.

## Direction

- **Page kinds we ship:** (e.g. marketing landing / app shell / docs)
- **Audience:**
- **Vibe (one line):**
- **References:** (URLs or products — optional)

## Visual rules

- Typography: distinctive display + readable body; avoid Inter/Roboto/Arial as the only stack unless justified below.
- Color: define CSS variables; avoid default purple-on-white AI gradient unless that *is* the brand.
- Layout: one job per section; hero is one composition, not a dashboard.
- Motion: 2–3 intentional moments max on marketing surfaces.

## Anti-slop (forbidden unless justified)

- Card-grid wallpaper with no hierarchy
- Emoji-as-icon clusters
- Pill/stat strips in the first viewport
- Generic stock “AI SaaS” chrome with no product signal

## Review

After UI changes, run **`cda-design-loop`** (visual) then **`cda-design-audit`** (checklist).
```

- [ ] **Step 3: Write `reference.md`**

Full rubric from the spec (Must / Should / Anti-slop). Include the severity labels `Blocker|High|Medium|Nitpick` and the rule: fix only Blocker+High inside the loop.

- [ ] **Step 4: Write `SKILL.md`**

Frontmatter:

```yaml
---
name: cda-design-loop
description: >-
  Closed visual design loop for rendered UI: reach the running app, screenshot
  key routes at 375/768/1440 when eyes exist, score against DESIGN.md, fix
  Blocker/High findings up to 3 times. Use after building or redesigning pages,
  on "design pass", "looks wrong", "review the page visually", or when a web
  worker finishes UI work and a project DESIGN.md exists.
---
```

Body must include, in order:

1. Announce: `Using cda-design-loop.`
2. Inputs resolution (brief → URL → routes → eyes) exactly as the spec.
3. Brand rule: curated brand only if evidenced / named / user asks; else taste stub / `templates/DESIGN.stub.md` via seed.
4. Loop protocol (cap 3) and exit criteria table.
5. Pointer to `reference.md`.
6. Sibling table (`cda-design-md`, taste, bold, audit, prototype).
7. Explicit: never claim visual PASS when eyes were SKIP; dead server → FAIL_SERVER not SKIP.

- [ ] **Step 5: Patch `cda-design-md` comparison table**

Add a row:

| **cda-design-loop** | After UI exists: visual pass vs project `DESIGN.md` |

And in Workflow step 5, prefer: run **cda-design-loop**, optionally **cda-design-audit**.

- [ ] **Step 6: Commit**

```bash
git add skills/design/cda-design-loop skills/design/cda-design-md/SKILL.md
git commit -m "$(cat <<'EOF'
feat(design): add cda-design-loop skill and taste DESIGN stub

EOF
)"
```

---

### Task 2: Seed script + shot template + tokens stub + tests

**Files:**
- Create: `skills/design/cda-design-loop/templates/tokens.stub.json`
- Create: `skills/design/cda-design-loop/templates/cda-design-shot.sh`
- Create: `skills/design/cda-design-loop/scripts/seed-design.sh`
- Create: `skills/design/cda-design-loop/scripts/tests/test_seed_design.sh`

**Interfaces:**
- Consumes: templates from Task 1; kit brands dir `skills/design/cda-design-md/brands/`
- Produces: CLI `seed-design.sh --target DIR [--force] [--no-tokens] [--no-shot] [--brand NAME]`
  - Exit 0 on success; non-zero on missing target / invalid brand
  - Brand copy **only** when `--brand` is passed **or** existing target `DESIGN.md` already names a known brand (detect case-insensitive: Linear, Notion, Cal, Raycast, Cursor, Resend) — otherwise write taste stub
  - Default: write shot script; `--no-shot` skips

- [ ] **Step 1: Write `templates/tokens.stub.json`**

```json
{
  "$schema": "https://design-tokens.github.io/community-group/format/",
  "color": {
    "primitive": {
      "ink": { "$value": "#0a0a0a" },
      "paper": { "$value": "#fafafa" },
      "accent": { "$value": "#0d9488" }
    },
    "semantic": {
      "fg": { "$value": "{color.primitive.ink}" },
      "bg": { "$value": "{color.primitive.paper}" },
      "focus": { "$value": "{color.primitive.accent}" }
    }
  },
  "font": {
    "primitive": {
      "display": { "$value": "\"Fraunces\", Georgia, serif" },
      "body": { "$value": "\"Source Sans 3\", system-ui, sans-serif" }
    },
    "semantic": {
      "heading": { "$value": "{font.primitive.display}" },
      "text": { "$value": "{font.primitive.body}" }
    }
  },
  "radius": {
    "sm": { "$value": "4px" },
    "md": { "$value": "8px" }
  }
}
```

(Stub values are examples — projects override; not a forced brand.)

- [ ] **Step 2: Write `templates/cda-design-shot.sh`**

Portable bash: usage `cda-design-shot.sh <URL> <OUT_DIR>`. For each width in 375 768 1440, write `OUT_DIR/w<width>.png`. If `npx`/`playwright` unavailable, print `SKIP_VISUAL: Playwright not available` to stderr and exit 2. Never `npm install` or download browsers automatically. Prefer `npx --no-install playwright` when possible; if that fails, exit 2 with the same SKIP message.

Minimal Playwright one-shot via node -e is acceptable **only when** `playwright` resolves; keep the script under ~80 lines.

- [ ] **Step 3: Write `scripts/seed-design.sh`**

```bash
#!/usr/bin/env bash
# seed-design.sh — drop DESIGN.md (+ optional tokens/shot) into a target repo.
set -euo pipefail
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# flags: --target (required), --force, --no-tokens, --no-shot, --brand NAME, -h
```

Behavior:

1. Resolve `--target` to absolute path; require it is a directory.
2. Decide brief source:
   - If `--brand NAME` and `brands/<mapped>/DESIGN.md` exists under `cda-design-md` → copy that.
   - Else if target already has `DESIGN.md` or `docs/DESIGN.md` and text matches a known brand name → keep file unless `--force` (on `--force` with detected brand, re-copy that brand).
   - Else if no brief exists (or `--force` with no brand) → copy `templates/DESIGN.stub.md` → `DESIGN.md` at target root.
3. Tokens: unless `--no-tokens`, write `design/tokens.json` from stub if missing or `--force`.
4. Shot: unless `--no-shot`, write `scripts/cda-design-shot.sh` from template (chmod +x) if missing or `--force`.
5. Print a one-line summary of what was written / skipped.

Brand directory map: `linear`→`linear.app`, `notion`→`notion`, `cal`→`cal`, `raycast`→`raycast`, `cursor`→`cursor`, `resend`→`resend`.

- [ ] **Step 4: Write failing/passing test `scripts/tests/test_seed_design.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail
# tests → scripts → cda-design-loop → design → skills → repo (5× ..)
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SEED="$ROOT/skills/design/cda-design-loop/scripts/seed-design.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/proj"
bash "$SEED" --target "$TMP/proj"
test -f "$TMP/proj/DESIGN.md"
test -f "$TMP/proj/design/tokens.json"
test -x "$TMP/proj/scripts/cda-design-shot.sh"
grep -q 'Taste stub' "$TMP/proj/DESIGN.md"
# idempotent: second run without --force must not fail
bash "$SEED" --target "$TMP/proj"
# --no-shot / --no-tokens
TMP2="$(mktemp -d)"
mkdir -p "$TMP2/proj"
bash "$SEED" --target "$TMP2/proj" --no-shot --no-tokens
test ! -e "$TMP2/proj/scripts/cda-design-shot.sh"
test ! -e "$TMP2/proj/design/tokens.json"
# --brand linear
TMP3="$(mktemp -d)"
mkdir -p "$TMP3/proj"
bash "$SEED" --target "$TMP3/proj" --brand linear --no-tokens --no-shot
grep -qi 'linear' "$TMP3/proj/DESIGN.md"
echo "test_seed_design: ok"
```

Fix `ROOT` path carefully: from `skills/design/cda-design-loop/scripts/tests` → `../../../../..` is repo root (tests→scripts→cda-design-loop→design→skills→repo = 5 levels). Use 5 `..` segments.

- [ ] **Step 5: Run the test**

```bash
bash skills/design/cda-design-loop/scripts/tests/test_seed_design.sh
```

Expected: `test_seed_design: ok`

- [ ] **Step 6: Commit**

```bash
git add skills/design/cda-design-loop
git commit -m "$(cat <<'EOF'
feat(design): add seed-design.sh and screenshot helper template

EOF
)"
```

---

### Task 3: Discovery docs + web overlay

**Files:**
- Modify: `skills/README.md`
- Modify: `AGENTS.md` (Skills by context table)
- Modify: `skills/core/cda-agents/templates/AGENTS.base.md`
- Modify: `skills/core/cda-agents/templates/overlays/web.md`
- Modify: `docs/recommended.md`
- Modify: `docs/superpowers/specs/2026-10-07-cda-skills-and-design-pack-design.md` (one-line cross-link)

**Interfaces:**
- Consumes: skill name `cda-design-loop`; seed path for humans
- Produces: agents discover loop + eyes; web overlay lists workflow order

- [ ] **Step 1: Update `skills/README.md` design row**

Include `cda-design-loop` in the design cell list.

- [ ] **Step 2: Update kit `AGENTS.md` + `AGENTS.base.md` skill tables**

Add:

| Visual design pass (screenshot loop) | `cda-design-loop` |

Keep existing design rows; place loop after audit or after bold/taste as “after UI exists”.

- [ ] **Step 3: Expand `overlays/web.md`**

Replace/extend the UI bullet with:

```markdown
- UI work: brief first (`DESIGN.md` via seed / `cda-design-md` only if brand already on project),
  then `cda-design-taste` or `cda-design-bold` to build, then **`cda-design-loop`**
  (visual), then `cda-design-audit` (checklist). Seed: `cda-design-loop/scripts/seed-design.sh --target .`
- Optional auto-loop from `cda-dev`: set `CDA_DESIGN_LOOP=1` or note `design-loop: auto` in Project-specific notes (off by default).
```

- [ ] **Step 4: Add Eyes section to `docs/recommended.md`**

```markdown
## Eyes for design agents (screenshots)

Agents need to **see** rendered pages for `cda-design-loop`. Prefer, in order:

1. Playwright MCP or Cursor browser tools already available in the session
2. Project script `scripts/cda-design-shot.sh` (from `seed-design.sh`) if Playwright is already installed locally
3. Otherwise the loop reports `SKIP_VISUAL` and still runs textual checks

Do not auto-install Playwright from the kit. Superdesign remains optional for image→code (see above).
```

- [ ] **Step 5: Cross-link from the 2026-10-07 design-pack spec**

Under Follow-up / Out of scope, add: Visual loop — see `2026-10-17-cda-design-loop-design.md`.

- [ ] **Step 6: Commit**

```bash
git add skills/README.md AGENTS.md skills/core/cda-agents docs/recommended.md \
  docs/superpowers/specs/2026-10-07-cda-skills-and-design-pack-design.md
git commit -m "$(cat <<'EOF'
docs(design): wire cda-design-loop into AGENTS and web overlay

EOF
)"
```

---

### Task 4: `cda-dev` opt-in auto-loop

**Files:**
- Modify: `skills/core/cda-dev/SKILL.md`
- Modify: `skills/core/cda-dev/prompts/worker.md`

**Interfaces:**
- Consumes: env `CDA_DESIGN_LOOP`, optional `--design-loop`, AGENTS note `design-loop: auto`
- Produces: workers invoke `cda-design-loop` after verify when opt-in **and** UI paths touched **and** `DESIGN.md`/`docs/DESIGN.md` exists; otherwise unchanged

- [ ] **Step 1: Add a short section to `cda-dev/SKILL.md`**

Under related skills / verify loop:

```markdown
### Optional design loop (`CDA_DESIGN_LOOP`)

Default: workers do **not** run `cda-design-loop`.
Opt-in when UI files change and a project brief exists, if any of:

- env `CDA_DESIGN_LOOP=1`
- `--design-loop` on orch / dispatch (when wired)
- `AGENTS.md` Project-specific notes contains `design-loop: auto`

UI path heuristic: `*.vue`, `*.tsx`, `*.jsx`, `*.css`, `*.scss`, or paths under `pages/`, `app/`, `components/`, `layouts/`.
```

- [ ] **Step 2: Insert step in `worker.md` after Test / verify, before Systematic review**

```markdown
3b. **Design loop (opt-in only)**
   - If `CDA_DESIGN_LOOP=1` or `AGENTS.md` notes `design-loop: auto` or dispatch passed `--design-loop`,
     and this diff touches UI paths, and `DESIGN.md` or `docs/DESIGN.md` exists:
     run skill `cda-design-loop` on affected routes; treat `FAIL_SERVER` / remaining Blocker|High after CAP as verify failures.
   - Otherwise skip.
```

Renumber following steps only if the file already uses strict numbering — prefer inserting as `3b` to minimize churn.

- [ ] **Step 3: Commit**

```bash
git add skills/core/cda-dev/SKILL.md skills/core/cda-dev/prompts/worker.md
git commit -m "$(cat <<'EOF'
feat(cda-dev): document opt-in cda-design-loop for UI diffs

EOF
)"
```

---

### Task 5: Install link check + verify list update

**Files:**
- Modify: `AGENTS.md` Project-specific notes verify block (add seed test)
- No change to `install.sh` logic (flatten-by-SKILL.md already picks up new skill)

- [ ] **Step 1: Extend verify commands in kit `AGENTS.md`**

```bash
for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh; do bash "$t"; done
bash skills/quality/cda-kiss/hooks/test.sh
bash skills/design/cda-design-loop/scripts/tests/test_seed_design.sh
```

- [ ] **Step 2: Dry-run install with temp homes**

```bash
HOME=/tmp/cda-design-loop-install \
CURSOR_HOME=/tmp/cda-design-loop-install/.cursor \
CLAUDE_CONFIG_DIR=/tmp/cda-design-loop-install/.claude \
XDG_CONFIG_HOME=/tmp/cda-design-loop-install/.config \
./install.sh --force --dry-run --no-hooks --cursor 2>&1 | grep cda-design-loop
```

Expected: a dry-run link line for `cda-design-loop`.

- [ ] **Step 3: Run full verify subset for this feature**

```bash
bash skills/design/cda-design-loop/scripts/tests/test_seed_design.sh
bash skills/quality/cda-kiss/hooks/test.sh
```

Expected: both ok.

- [ ] **Step 4: Mark spec status confirmed + commit**

Ensure `docs/superpowers/specs/2026-10-17-cda-design-loop-design.md` says `Status: approved`. Commit any remaining doc/verify edits:

```bash
git add AGENTS.md docs/superpowers/specs/2026-10-17-cda-design-loop-design.md
git commit -m "$(cat <<'EOF'
chore(design): verify seed test in kit AGENTS checklist

EOF
)"
```

---

## Spec coverage checklist

| Spec requirement | Task |
| ---------------- | ---- |
| Skill protocol + exit criteria | 1 |
| Rubric in reference | 1 |
| Taste stub; brand only if evidenced / `--brand` | 1–2 |
| Shot script default; `--no-shot` | 2 |
| Tokens optional; `--no-tokens` | 2 |
| Seed idempotent + test | 2 |
| Docs / AGENTS / web overlay / recommended eyes | 3 |
| `cda-dev` opt-in auto-loop | 4 |
| No Playwright kit dependency; install discovers skill | 5 |

## Out of plan (later)

- DTCG ↔ Tailwind sync helper
- Vision-API critic flag
- Wiring `--design-loop` into `orch.sh` CLI flags (document in Task 4; implement CLI only if already trivial — otherwise env + AGENTS note is enough for v1)

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-10-17-cda-design-loop.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — same session with executing-plans and checkpoints  

Which approach?
