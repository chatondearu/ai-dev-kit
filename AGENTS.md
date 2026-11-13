# Agent guide

<!-- agents-setup:managed -->

## Mandatory workflow

Every feature, fix, or decision that affects product, architecture, or design
must follow this sequence:

1. Follow the foam protocol (`cda-foam` § Protocol): read this file and
   `foam/index.md`, search ADRs/PRDs/plans and GitHub issues/PRs, confront the
   proposal with recorded decisions, cite sources.
2. Any feature or fix requested in chat — one subject or several — goes through
   `cda-dev` intake (`cda-dev/prompts/intake.md`): one round of questions, a
   breakdown, GO. A single child issue is fine. Do not start coding from chat,
   except a trivial fix (typo, one-liner, obvious small fix): branch → commit → PR.
3. For non-trivial scope, before coding a Kanban issue, `prd/PRD-<n>-*.md` and
   `plans/PLAN-<n>-*.md` exist (create the missing ones). A fast-lane Ready
   issue may use its triage agent brief instead (`cda-dev`, `cda-triage`).
4. After shipping or deciding: update foam (ADR, PRD status, feature lists) and
   refresh this file when stack or skills change.

Skills: **cda-foam**, **cda-prd**, **cda-plan**,
**cda-dev**, **cda-graph**, **cda-kanban**, **cda-doc-hygiene**,
**cda-agents**, **cda-commit**, **cda-review**.

## Project memory (foam)

| Path | Purpose |
| ---- | ------- |
| `foam/index.md` | Map of Content — start here |
| `foam/decisions/` | Architecture Decision Records |
| `prd/` | Product requirements |
| `plans/` | Implementation checklists |
| GitHub Project | Kanban execution status |

Sync feature lists: `cda-foam/scripts/import-kanban.sh`.

Cursor rule (if present): `.cursor/rules/product-memory.mdc`.

## Skills by context

| When | Skill |
| ---- | ----- |
| Bootstrap / update this file | `cda-agents` |
| ADRs, foam graph, import kanban | `cda-foam` |
| PRD from Kanban issue | `cda-prd` |
| Implementation plan from PRD | `cda-plan` |
| Feature / fix request in chat (one or many) → intake, PRs | `cda-dev` |
| Interactive cdamux orchestrator session | `cda-orchestrator` |
| Graph orchestration (parallel workers, graph-plan) | `cda-graph` |
| Issues, board, PRs, milestones | `cda-kanban` |
| Foam / PRD / plan layout verify | `cda-doc-hygiene` |
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

- Project toolchain: host/direnv first, `nix develop -c` only as fallback
  (**nix-develop-shell**). Never wrap `git`/`gh`/OS tools just because a
  `flake.nix` exists.
- New projects: **nix-direnv-setup** for flake + direnv scaffold.
- Deployment / hosting decisions → ADR in `foam/decisions/`.

<!-- /agents-setup:managed -->

## Project-specific notes

_Add stack versions, deployment URLs, gotchas, and links below._

This repo **is** the ai-dev-kit: skills under `skills/<category>/<name>/`, linked into
agents by `install.sh`. Index: `skills/README.md`. Ideology: `PHILOSOPHY.md`.
Operator docs: `doc/README.md` (orchestration, architecture, configuration).
External tools list: `docs/recommended.md` only — do not create `docs/superpowers/`
or PRDs/plans outside `prd/` / `plans/` (see `foam/product/documentation-layout.md`).
Closed issue plans: `foam/archive/plans/`.

**cdamux** (Rust agent/tmux orchestrator, ADR-004): in-tree at `cdamux/` until v0;
see `cdamux/AGENTS.md` and `cdamux/docs/research.md`. Nix package: `nix build .#cdamux`
(opt-in `cda.aitools.cdamux.*`, see `aitools/README.md`). Interactive session:
`cdamux orch new` + skill `cda-orchestrator` (not `orch.sh`).

Experimental opt-in companion: `aitools/` (`cda.aitools` Nix modules, ADR-003).
Never enabled by `install.sh`. Relocatable root: `CDA_AITOOLS_HOME`.

**Verify (run before every PR):** prefer host tools when the verify toolchain is
already on `PATH`; otherwise `nix develop -c bash scripts/verify.sh`.

```bash
bash scripts/verify.sh
# fallback if cargo/jq/… missing:
nix develop -c bash scripts/verify.sh
```

**Gotchas**

- Never run `install.sh` against the real user config from a test or a worker:
  override `HOME`, `CLAUDE_CONFIG_DIR`, `CURSOR_HOME`, `XDG_CONFIG_HOME` with temp dirs.
- The live Claude config on the maintainer's machine is `CLAUDE_CONFIG_DIR=/cda/.claude`.
- The kiss hooks run on every session: keep them local-only, fail-open, fast.
- cdamux session hierarchy (ADR-007): `install.sh --cdamux-hooks` (or Nix
  `cda.aitools.cdamux.hooks.enable`) is opt-in and plain `install.sh` never wires it; tests of `ORCH_TMUX=1` use a throwaway tmux server
  (`TMUX_TMPDIR` temp dir + `-L <name>`), never the live one.
- AFK Ready pickup: `orch-auto.sh` + optional systemd templates under
  `skills/core/cda-dev/templates/systemd/` (ADR-002). Do not enable from
  `install.sh`. Stop the timer to free the machine; prefer a dedicated host.
  Set `ORCH_AGENT` in its env file: with no parent session it exits 2 (ADR-010).
- `headless.sh` does **not** open PRs from issues (ADR-001); use orch / orch-auto.
- This repo has **no** GitHub Actions PR checks. Orch treats empty checks as green when:
  orch: allow-no-checks
  (or `ORCH_ALLOW_NO_CHECKS=1`). Poll live worker state under `$CDAMUX_HOME/runs/`
  (and agents); `.orch/run/` keeps work artefacts only.

verify: bash scripts/verify.sh (or nix develop -c bash scripts/verify.sh)
