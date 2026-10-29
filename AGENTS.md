# Agent guide

<!-- agents-setup:managed -->

## Mandatory workflow

Every feature, fix, or decision that affects product, architecture, or design
must follow this sequence:

1. Read this `AGENTS.md`, then `foam/index.md` (if present).
2. Search **foam** (`foam/decisions/`, `prd/`, `plans/`) for related ADRs and specs.
3. Search **GitHub** for overlap before starting work:
   ```bash
   gh issue list --state all --search "keywords" --limit 20
   gh pr list --state all --search "keywords" --limit 20
   ```
4. If an issue, PR, ADR, or PRD already covers the topic — **read it** and
   confront the new proposal with recorded decisions. Cite sources.
5. For non-trivial scope: ensure a Kanban issue, PRD (`prd/`), and plan
   (`plans/`) exist or create them before coding.
6. After shipping or deciding: update foam (ADR, PRD status, feature lists) and
   refresh this file when stack or skills change.

Skills: **cda-foam**, **cda-prd**, **cda-plan**,
**cda-dev**, **cda-graph**, **cda-kanban**,
**cda-agents**, **cda-commit**, **cda-review**.

## Project memory (foam)

| Path | Purpose |
| ---- | ------- |
| `foam/index.md` | Map of Content — start here |
| `foam/decisions/` | Architecture Decision Records |
| `prd/` | Product requirements |
| `plans/` | Implementation checklists |
| GitHub Project | Kanban execution status |

Operator docs: `doc/`. External tools list: `docs/recommended.md` only — do not
create `docs/superpowers/` or PRDs/plans outside `prd/` / `plans/` (see
`foam/product/documentation-layout.md`).

Sync feature lists: `cda-foam/scripts/import-kanban.sh`.

Cursor rule (if present): `.cursor/rules/product-memory.mdc`.

## Skills by context

| When | Skill |
| ---- | ----- |
| Bootstrap / update this file | `cda-agents` |
| ADRs, foam graph, import kanban | `cda-foam` |
| PRD from Kanban issue | `cda-prd` |
| Implementation plan from PRD | `cda-plan` |
| Multi-task chat batch → epic + parallel PRs | `cda-dev` |
| Issues, board, PRs, milestones | `cda-kanban` |
| Commits | `cda-commit` |
| Code review | `cda-review` |
| Bold creative UI | `cda-design-bold` |
| Anti-slop landings / redesigns | `cda-design-taste` |
| Brand DESIGN.md templates | `cda-design-md` |
| UI a11y / UX audit | `cda-design-audit` |
| Visual design pass (screenshot loop) | `cda-design-loop` |
| User-facing `doc/` | `cda-docs` |
| Library docs | `cda-context7` |
| Nix / direnv projects | `nix-develop-shell`, `nix-direnv-setup` |


## Stack — Infrastructure / Nix

- Run project commands inside `nix develop` (**nix-develop-shell**).
- New projects: **nix-direnv-setup** for flake + direnv scaffold.
- Deployment / hosting decisions → ADR in `foam/decisions/`.

<!-- /agents-setup:managed -->

## Project-specific notes

_Add stack versions, deployment URLs, gotchas, and links below._

This repo **is** the ai-dev-kit: skills under `skills/<category>/<name>/`, linked into
agents by `install.sh`. Index: `skills/README.md`. Ideology: `PHILOSOPHY.md`.
Operator docs: `doc/README.md` (orchestration, architecture, configuration).

**cdamux** (Rust agent/tmux orchestrator, ADR-004): in-tree at `cdamux/` until v0;
see `cdamux/AGENTS.md` and `cdamux/docs/research.md`. Not packaged in `aitools` yet.

Experimental opt-in companion: `aitools/` (`cda.aitools` Nix modules, ADR-003).
Never enabled by `install.sh`. Relocatable root: `CDA_AITOOLS_HOME`.

**Verify (run before every PR):**

```bash
for t in skills/core/cda-dev/scripts/tests/test_*.sh skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh scripts/tests/test_doctor.sh; do bash "$t"; done
bash skills/quality/cda-kiss/hooks/test.sh
bash skills/design/cda-design-loop/scripts/tests/test_seed_design.sh
(cd cdamux && cargo test)
```

**Gotchas**

- Never run `install.sh` against the real user config from a test or a worker:
  override `HOME`, `CLAUDE_CONFIG_DIR`, `CURSOR_HOME`, `XDG_CONFIG_HOME` with temp dirs.
- The live Claude config on the maintainer's machine is `CLAUDE_CONFIG_DIR=/cda/.claude`.
- The kiss hooks run on every session: keep them local-only, fail-open, fast.
- AFK Ready pickup: `orch-auto.sh` + optional systemd templates under
  `skills/core/cda-dev/templates/systemd/` (ADR-002). Do not enable from
  `install.sh`. Stop the timer to free the machine; prefer a dedicated host.
- `headless.sh` does **not** open PRs from issues (ADR-001); use orch / orch-auto.
- This repo has **no** GitHub Actions PR checks. Orch treats empty checks as green when:
  orch: allow-no-checks
  (or `ORCH_ALLOW_NO_CHECKS=1`). Poll live worker state at `.orch/run/status.json`.

verify: bash -n skills/core/cda-kanban/scripts/gh-board.sh && bash skills/core/cda-kanban/scripts/gh-board.sh reconcile-backlog --help
