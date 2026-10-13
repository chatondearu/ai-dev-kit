# Philosophy

What this kit is for, and where its ideas come from.

## Ideology

**Project-first, memory-backed.** Every repo runs on a portable contract
(`AGENTS.md`), a versioned memory graph (`foam/` — ADRs, PRDs, plans) and a
GitHub Kanban board. Before any feature or product/architecture/design decision,
an agent reads the memory, searches open issues/PRs, and **confronts the proposal
against recorded decisions** instead of re-deciding from scratch. The pipeline is
explicit: **explore (clear the fog) → PRD → plan → orchestrate (workers in
worktrees, verify loops, CI-green PRs, never auto-merge) → review / test / docs →
write it back to memory.** One dependency language (`Blocked by:`), one
out-of-scope register, one behavioral-brief contract run across the whole pipeline
so a decision keeps its meaning from intake to implementation.

**KISS as an always-on code attitude.** The best code is the code you never
wrote. Smallest change that fully solves the task, reuse before writing, and a
reply a busy human reads in one pass — **never** at the cost of validation,
security, accessibility or error handling. This attitude is injected into every
session and every subagent via hooks, not left to memory.

**Quality is enforced, not hoped for.** Reviews carry explicit lenses (standards,
spec-conformance, leanness/reuse, security); risky logic ships with a test; docs
and decision records are gated so they can't silently drift; a whole-repo audit
ranks what to fix first.

**Tool-agnostic and portable.** One canonical `SKILL.md` tree, symlinked into
Cursor / Claude Code / opencode by `install.sh`. Kit-owned workflow skills carry
the `cda-` prefix; adapted third-party skills keep a clear lineage.

**Steal the idea, re-author it, credit the source.** When an external project has
a sharp idea, we take the idea — re-written to fit this kit — and deliberately
leave the bloat behind (marketing, packaging, 20-tool matrices, i18n). Sources are
credited below and in [`THIRD_PARTY.md`](THIRD_PARTY.md).

## Inspirations

### [mattpocock/skills](https://github.com/mattpocock/skills) — *skills for real engineers* (MIT)

The engineering-practice backbone. Directly adapted into the kit:

- **Practices** → `tdd`, `diagnosing-bugs`, `grilling`, `prototype`, `retro`,
  `wizard`, `writing-for-agents`, `codebase-design`,
  `improve-codebase-architecture`, and `review-standards-spec` (the two-axis
  Standards ∥ Spec review).
- **Pipeline ideas folded into `cda-*`**: tracer-bullet tickets with first-class
  **blocking edges / frontier** dispatch (`cda-dev`), durable **agent-ready
  behavioral briefs** (`cda-dev`, `cda-triage`), the **out-of-scope register**
  (`cda-foam`), `wayfinder` → **`cda-explore`**, and `triage` → **`cda-triage`**.

We skipped its tracker-coupling (`setup-matt-pocock-skills`) and anything specific
to its own stack.

### [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) — *the laziest senior dev* (MIT)

The KISS code-attitude layer. Re-authored into the kit:

- **`kiss`** — the ruleset (smallest complete change, reuse-first ladder, never
  cut the essentials) delivered **always-on via lifecycle hooks**: session +
  subagent injection, a `git ls-files` repo map, `/kiss lite|full|ultra|off`
  toggle, and a statusline badge.
- **`kiss-audit`** — whole-repo ranked quality audit (must-fix / should-fix /
  nice-to-have).
- The **leanness & reuse lens** grafted into `code-reviewer` and the `cda-dev`
  reviewer.

We took the attitude + the hook engine and left the product shell: marketplace/npm
packaging, auto-update, the ~20-agent matrix, i18n, benchmarks and the marketing
site. We audited its hooks before re-authoring — local-only, no network, fail-open.

---

Both upstreams are MIT. This kit's adaptations are re-written, not vendored
verbatim; see each skill's `SKILL.md` for inline credit where applicable.
