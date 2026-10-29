---
name: cda-doc-hygiene
description: >-
  Verify and clean foam/PRD/plan/doc layout per cda-foam conventions: forbidden
  paths (docs/superpowers), misplaced artifacts, broken plan links, optional GitHub
  archive of closed-issue plans. Use before doc cleanup, after Superpowers sessions,
  or when onboarding project memory on a repo.
---

# Doc hygiene (foam layout)

Announce: **"Using cda-doc-hygiene skill."**

## Role

Machine + agent guardrails for **project memory layout** (not runtime `doc/` sync —
use **cda-docs** for that).

Canonical rules: `foam/product/documentation-layout.md`, skill **cda-foam**.

## Script

```bash
SKILL=~/.cursor/skills/cda-doc-hygiene
bash "$SKILL/scripts/doc-hygiene.sh" --check          # CI / verify (exit 1 on FAIL)
bash "$SKILL/scripts/doc-hygiene.sh" --report         # markdown report for chat
bash "$SKILL/scripts/doc-hygiene.sh" --check --gh     # + warn closed issues still in plans/
bash "$SKILL/scripts/doc-hygiene.sh" --archive-done --dry-run
bash "$SKILL/scripts/doc-hygiene.sh" --archive-done   # git mv closed plans → foam/archive/plans/
```

From ai-dev-kit tree:

```bash
bash skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh --check
```

`DOC_HYGIENE_STRICT=1` — treat warnings as exit 1.

## Agent workflow

1. Run `--report`; cite each FAIL/WARN.
2. **Fix violations** — never create `docs/superpowers/` or PRDs outside `prd/`.
3. Optional: `cda-foam` → `import-kanban.sh` if feature indexes are stale.
4. Optional: user confirms → `--archive-done --dry-run`, then `--archive-done`;
   update PRD/`AGENTS.md` links from `plans/` to `foam/archive/plans/`.
5. Do **not** delete shipped PRDs or merge plans without user approval.
6. Runtime docs: **cda-docs** check mode if code changed.

## Bootstrap other repos

1. **cda-agents** → `setup-agents.sh --init-foam`
2. Ensure `foam/product/documentation-layout.md` exists (from foam scaffold).
3. Add to `AGENTS.md` § Verify: `doc-hygiene.sh --check`

## Integration

| Skill | Role |
| ----- | ---- |
| `cda-foam` | Layout source, import-kanban |
| `cda-prd` / `cda-plan` | Create artifacts at correct paths |
| `cda-docs` | Operator `doc/` sync |
| `cda-dev` | Orch reads `plans/PLAN-<id>-*` only (archive closed plans) |

## Checks (summary)

- Forbidden: `docs/superpowers/`, `.superpowers/sdd`
- `PRD-*.md` only under `prd/` (+ skill templates)
- `PLAN-*.md` only under `plans/` or `foam/archive/plans/` (+ archive/)
- Broken backtick links to missing `plans/PLAN-*.md`
- If `foam/`: require `foam/index.md`; warn if layout doc missing
