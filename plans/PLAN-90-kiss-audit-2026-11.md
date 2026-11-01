# PLAN-90 — Fix kiss-audit 2026-11-01 with a fleet of subagents

- **Status**: draft
- **PRD**: [[../prd/PRD-90-kiss-audit-2026-11|PRD-90]]
- **GitHub**: [#90](https://github.com/chatondearu/ai-dev-kit/issues/90)
- **Date**: 2026-11-01

## Summary

Close the 20 findings of the 2026-11-01 audit by launching **17 subagents in
3 lanes** (harness, skills, cdamux), each in its own git worktree, each opening
one PR. The model is chosen per task difficulty: frontier model only where
judgment or safety is at stake, mid model for bounded code changes with tests,
fast model for mechanical edits. The operator reviews PRs and merges; nothing
auto-merges.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | `headless.sh` never opens PRs from issues; host tools stay outside the kit |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | `orch-auto` is opt-in, never enabled by `install.sh`; no auto-merge |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux is the source of truth; daemon optional; tailnet-only exposure, never `0.0.0.0` |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Graph = workers → integrator → verifier; verifier grounds on plan/ADR |

Do not violate accepted ADRs without superseding them. Task H2 adds **ADR-006**
(independent review gate + fail-closed verify) because it changes the orch
contract.

## Scope

### In scope

Audit findings 1–20 (see PRD). Child issues, one per task below.

### Out of scope

- Auto-merge or merge babysitting
- A model knob inside `orch.sh` (`cursor-agent --model`) — the fleet below is
  launched from a Cursor session with the `Task` tool, which already selects the
  model; add the knob later if `orch-auto` needs it
- Web UI, cdamux extraction (#56), design/productivity skill bodies

## Technical approach

**Why `Task` subagents and not `orch.sh`:** the orch cannot be trusted to fix
itself — its verify is the thing that is broken (finding 2) and it has no model
selection. Each task is launched as a `Task` subagent from the operator's Cursor
session, with the model from the matrix, inside a worktree created with the
kit's own helper so paths match what `orch-cleanup.sh` expects.

**Isolation:** one worktree + one branch per task:
`.orch/worktrees/<task-id>-issue-<child#>`, branch `fix/<child#>-<slug>`.
A task owns the files listed in its row; it must not edit files owned by another
task in the same lane wave. Before opening the PR the subagent rebases on
`origin/main`.

**Lanes** (parallel across lanes, ordered inside a lane by the *Depends* column):

- **H** — harness `skills/core/cda-dev/` + `AGENTS.md` + tests
- **S** — skills, rules, prompts text, docs
- **C** — `cdamux/` Rust

**Model matrix**

| Tier | Model (Cursor `Task` slug) | Use when |
| ---- | -------------------------- | -------- |
| frontier | `inherit` (Claude Fable 5.1) | judgment, safety, or a prompt whose wording decides agent behavior |
| mid | `claude-sonnet-5-5-high` | bounded code change + one test, several files |
| fast | `composer-2.5-fast` | mechanical edit, one or two files, verified by an existing test or `bash -n` / `cargo test` |

## Tasks

Legend — **F#** = audit finding. **Verify** is what the subagent must run green
before opening the PR, in addition to the AGENTS.md Verify block.

### Wave 0 — enablers (run first, both lanes H and C can start right after)

| Id | F# | Task (behavioral contract) | Owns | Model | Depends | Verify |
| -- | -- | -------------------------- | ---- | ----- | ------- | ------ |
| H1 | 9 | `smoke_orch_dry.sh` passes on `main`: the stub `gh` answers `repo view --json nameWithOwner`. The `verify:` line at the bottom of `AGENTS.md` runs the **whole** "Verify (run before every PR)" block (one script or a `for` loop), so the orch executes the real suite. | `cda-dev/scripts/tests/smoke_orch_dry.sh`, `AGENTS.md` (verify line only), new `scripts/verify.sh` if needed | fast | — | `bash smoke_orch_dry.sh`; the `verify:` command exits 0 |

- [ ] H1 merged

### Wave 1 — Must fix (parallel: H2, S1, S2, C1, C3)

| Id | F# | Task | Owns | Model | Depends | Verify |
| -- | -- | ---- | ---- | ----- | ------- | ------ |
| H2 | 1, 2 | **Quality gate.** After the worker loop, `reviewer.md` is rendered and run in a fresh agent context; `changes_requested` triggers exactly one fix iteration then `blocked`. `ORCH_GRAPH_VERIFY` defaults to `1`. `orch_resolve_verify` prefers a fenced "Verify" block over a `verify:` line; no command found → `blocked` unless the issue body has `orch: allow-no-verify`. The retry prompt carries the last 40 lines of the failed verify. `verifier.md` fails on added `TODO|FIXME|\.skip\(|xit\(` or removed test files not named in the plan. Record **ADR-006**. | `orch.sh` (ready-pickup path), `lib/verify.sh`, `lib/loop.sh`, `prompts/{reviewer,verifier}.md`, `conventions.md` §verify, `foam/decisions/ADR-006-*.md`, tests `test_verify*.sh` | frontier | H1 | new test: AGENTS.md with both a `verify:` line and a Verify block → block wins; no command → `blocked`; dry ready-pickup shows a reviewer step |
| S1 | 5 | Global Claude rules no longer contradict the worker: `rules/00-communication.md` gains one exception line for cda-dev worker/pr-fix prompts (commit + push on the worker branch, English artifacts); the "tests only when requested" rule is aligned with `cda-kiss` (one test per non-trivial logic). | `rules/00-communication.md`, `rules/02-adhd-output.md` | mid | — | `rg -n "cda-dev worker" rules/` ≥ 1; `install.sh` test suite green |
| S2 | 3 | **Routing + questioning.** `AGENTS.base.md` § Mandatory workflow routes any feature/fix request, one subject or many, to `cda-dev` intake; `intake.md` accepts one or more subjects and delegates questioning to `cda-grill` (one frontier round, recommended answer per question); `rules/02-adhd-output.md` keeps its exception only for a single ambiguous word. Three policies become one. | `skills/core/cda-agents/templates/AGENTS.base.md`, this repo's `AGENTS.md` (managed block), `cda-dev/SKILL.md` §intake, `cda-dev/prompts/intake.md`, `cda-grill/SKILL.md` (one link line), `rules/02-adhd-output.md` §questions | frontier | — | `cda-agents` tests green; `rg -n "one question at a time"` → 0 outside grill |
| C1 | 6, 7 | **tmux targeting & send.** `send_to_session` delegates to `tmux::send_line_to_session` (two invocations, `-l` text then `Enter`); `build_send_keys_argv` removed and its test replaced. `session_name_for_path` replaces `.` and `:` with `_`. `tmux_has_session` uses the exact-match prefix `=`. `apply_pane_split` targets the session (active pane) and no longer sends `$SHELL`. | `cdamux-core/src/{send,spawn}.rs`, `session_manage.rs` call sites, tests | mid | — | `cargo test`; manual on a throwaway server `tmux -L audit`: `cdamux send t1 "echo hi" --enter` executes; dir `web.app` spawns twice; `base-index 1` split works |
| C3 | 8 | **Tailnet hardening.** `resolve_tailnet_bind_ip` rejects unspecified IPs with a clear error; `resolve_orch_repo_path` canonicalizes before `starts_with` and never echoes the path in 404/422 bodies; `handle_one` requires header `X-Cdamux-Token` equal to `CDAMUX_TAILNET_TOKEN` when set, documented in `cdamux/docs/tailnet-api.md` and the ADR-004 amendment. | `cdamux-core/src/tailnet_api.rs`, `cdamuxd/src/tailnet_server.rs`, `cdamux/docs/tailnet-api.md`, `foam/decisions/ADR-004-cdamux.md` (amendment section) | frontier | — | tests: `0.0.0.0` → Err; `"/root/../../etc"` → 404; missing token → 401; `/v1/orch-graph?repo=` dispatch exercised |

- [ ] H2 merged (ADR-006 accepted)
- [ ] S1 merged
- [ ] S2 merged
- [ ] C1 merged
- [ ] C3 merged

### Wave 2 — Should fix (parallel: H3, H4, S3, S4, C4, C5)

| Id | F# | Task | Owns | Model | Depends | Verify |
| -- | -- | ---- | ---- | ----- | ------- | ------ |
| H3 | 4, 13 | **Prompt context + contracts.** `{{PRD_PATH}}` and `{{EPIC}}` are resolved by reusing the lookup already in `cda-plan/scripts/new-plan.sh` (PRD by issue, `## Parent`); `{{CONSTRAINTS}}` lists `foam/decisions/ADR-*.md`; graph verifier no longer hard-codes `N/A`. `worker.md` steps renumbered, duplicate preamble step removed; `conventions.md` example `worker-result` includes `checks` and states frontier is fail-closed; `foam/plans` globs and the "ADR-017" comment removed. | `orch.sh` (render call only), `lib/graph.sh` (render call + globs), `lib/loop.sh` (new `orch_resolve_context`), `prompts/worker.md`, `conventions.md`, `cda-plan/scripts/new-plan.sh` (extract helper if needed) | mid | H2 | rendered worker prompt on this repo for #90 shows `prd/PRD-90-…` and ADR list; `rg "foam/plans" scripts/` → 0 |
| H4 | 10 | **graph-plan limits.** `max_parallel` is honored (nested default fixed); after a node's worker, `git diff --name-only` outside the node's declared `paths` → node `blocked` with the offending files listed. | `lib/graph.sh` (fan-out + post-worker check), `tests/test_graph*.sh` | mid | H2 | test: plan with `max_parallel: 1` never runs two workers; test: diff outside paths → blocked |
| S3 | 14 | Skills called headless never wait for a human: `cda-review`, `cda-tdd`, `cda-plan` gain one "unattended (worker)" line (preflight unconditional, seams = plan Test plan). Phantom skill names (`babysit`, `writing-plans`, `executing-plans`, `subagent-driven-development`, `code-reviewer`, `deep-research`) replaced by `cda-dev` / `cda-plan` / `cda-review` or removed. | `cda-review/SKILL.md`, `cda-tdd/SKILL.md`, `cda-plan/SKILL.md`, `cda-kanban/SKILL.md`, `cda-standards/SKILL.md`, `cda-explore/SKILL.md` | fast | — | `rg -n "babysit\|writing-plans\|executing-plans\|subagent-driven\|code-reviewer skill\|deep-research" skills/` → 0 |
| S4 | 15 | Cursor side is honest: `install.sh` stops copying `rules/` to `~/.cursor/user-rules/` (or documents the manual paste in `doc/`), `cda-kiss/SKILL.md` describes the real Cursor hook coverage (`sessionStart` + `beforeSubmitPrompt`, no subagent injection), README/doc updated. | `install.sh` (rules copy only), `skills/quality/cda-kiss/SKILL.md`, `doc/configuration.md` | fast | — | `scripts/tests/test_doctor.sh` + install tests green with temp `HOME` |
| C4 | 16 | **Daemon IPC removal.** `sessions_for_list` = local `enrich_sessions(list_tmux_sessions())`; `daemon_list_sessions`, `handle_daemon_request`, `rekey_daemon_sessions`, state cache read/write, `DaemonRequest/DaemonListResponse`, socket bind/accept removed. `poll_sessions` kept for the tailnet server. `doc`/`AGENTS.md` of cdamux updated ("daemon recommended" → "daemon = tailnet only"). | `cdamux-core/src/daemon.rs`, `cdamuxd/src/main.rs`, `cdamux/AGENTS.md`, `cdamux/docs/architecture.md` | mid | — | `cargo test`; `cdamux list` with daemon running returns in < 300 ms for 10 sessions |
| C5 | 17 | **TUI hot loop.** `flat_sessions`, `discover_projects`, agent lines are computed only under the refresh gate; `session_agent_label` tries `@ccmux_agent` only if `@cdamux_agent` is empty; one `tmux list-panes -a -F` call replaces per-session `display-message` + `list-panes`, parsed once. Target ≤ `2 + N` tmux spawns per Fleet tick. | `cdamux-cli/src/tui.rs` (loop only), `cdamux-core/src/{tmux,session,session_chrome}.rs` | frontier | — | `cargo test`; count spawns with `strace -f -e execve -c` or a `CDAMUX_TRACE_TMUX=1` counter over 10 s with 10 sessions |

- [ ] H3 merged
- [ ] H4 merged
- [ ] S3 merged
- [ ] S4 merged
- [ ] C4 merged
- [ ] C5 merged

### Wave 3 — Should fix, second pass (parallel: H5, S5, C6, C7)

| Id | F# | Task | Owns | Model | Depends | Verify |
| -- | -- | ---- | ---- | ----- | ------- | ------ |
| H5 | 11, 12 | **Process hygiene.** `orch.sh` and `graph.sh` trap `INT TERM` and kill their process group; `orch-auto.sh` exits non-zero (75) when the lock is held; dead `orch_wait_for_slot` deleted; the `wait -n` pool lives once in `lib/`; worktree root resolved with `git rev-parse --show-toplevel` so nested worktrees are impossible; `orch-cleanup.sh` runs after a successful run. | `orch.sh`, `orch-auto.sh`, `lib/{graph,worktree,loop}.sh`, `orch-cleanup.sh`, tests | mid | H3, H4 | test: Ctrl-C in dry run leaves no child; test: lock held → exit 75; test: run from inside a worktree creates `.orch/` at toplevel |
| S5 | 19 | **Dedupe skills.** Foam protocol lives once in `cda-foam` (others link); pipeline issue→PRD→plan→PR once in `cda-plan`; review checklist once in `cda-review` (reviewer.md links); the 10 "Announce: Using X skill" lines removed; vague verbs ("Adequate", "Ensure", "appropriate", "Consider") replaced by observable predicates in `reviewer.md`, `intake.md`, `cda-dev/SKILL.md`, `AGENTS.base.md`, `cda-review`. | `skills/**/SKILL.md` text, `cda-dev/prompts/*.md`, `.cursor/rules/product-memory.mdc` | mid | S2, S3 | `rg -n "Announce at start" skills/` → 0; `doc-hygiene.sh --check` green; total SKILL+prompts lines reported in PR (target −400) |
| C6 | 18 | **Dead code + duplicates.** Remove `cmd_spawn_interactive`, `spawn_modal::run` + its `TerminalGuard`, inline `tmux_attach_session`; `session_chrome::tmux_display` → `tmux_user_option`; `git_output` → `worktree.rs` helper; single `centered_rect`; `cmd_doctor` prints `doctor_agent_lines`; single `format_age`. | `cdamux-cli/src/{main,spawn_modal,tui,orch_graph}.rs`, `cdamux-core/src/{session_chrome,worktree}.rs` | fast | C5 | `cargo test`; `cargo build` with 0 dead-code warnings; `cargo clippy` count not higher than before |
| C7 | 18 | `cdamux/prototype/dashboard-ui/` moves to `foam/archive/cdamux-prototype/` with a one-line README pointing to PRD-41; `tui.rs` comment and `prompts/next-agent.md` updated. | `cdamux/prototype/`, `foam/archive/`, `cdamux/prompts/next-agent.md`, `plans/PLAN-41-*.md` link | fast | — | `rg -n "prototype/dashboard-ui" --glob '!foam/archive/**'` → 0 |

- [ ] H5 merged
- [ ] S5 merged
- [ ] C6 merged
- [ ] C7 merged

### Wave 4 — Nice to have + closure (parallel: C8, V1)

| Id | F# | Task | Owns | Model | Depends | Verify |
| -- | -- | ---- | ---- | ----- | ------- | ------ |
| C8 | 18 | `run_dashboard` split without new abstractions: pure tree model (`TreeRow`, `flatten_tree`, selection) → `fleet_tree.rs`; `draw_*` → `fleet_draw.rs`. Zero behavior change. | `cdamux-cli/src/tui.rs` → new modules | mid | C5, C6 | `cargo test` unchanged count; `tui.rs` < 900 lines |
| V1 | 20, all | **Closure verifier.** Re-run every repro of the audit report (harness `_orch_extract_agents_verify`, `max_p`, frontier, tmux send/`.`/`=`/split, `0.0.0.0`, `..`), confirm each is covered by a test; run the AGENTS.md Verify block + `cargo test`; update PRD-90 → `shipped`, this plan → `done`, `foam/features/shipped.md` via `import-kanban.sh`, `plans/README.md`. Opens one PR with foam updates only, or a comment on #90 listing uncovered repros. | foam, prd, plans | frontier | everything above merged | all checks green on `main`; report attached to #90 |

- [ ] C8 merged
- [ ] V1 done — PRD shipped, plan done

## Execution procedure

1. **Bootstrap children** (operator or one fast subagent): for each row, 
   `gh issue create --title "<type>(<area>): <Id> <short title>" --label type:fix|type:chore[,area:cdamux] --body "Part of #90 — PLAN-90 task <Id>. Finding F#. <Task cell>. Owns: <Owns cell>. Verify: <Verify cell>."`
   then `gh-board.sh` → Ready. Write the child number back in the table above.
2. **Launch a wave**: for each task in the wave, from the Cursor session, `Task`
   with `subagent_type: generalPurpose`, `model` from the matrix, prompt =
   template below. Launch all tasks of a wave in the same message so they run
   in parallel. Lanes H/S/C may overlap waves as soon as their *Depends* are
   merged.
3. **Review** each PR (operator). Merge order inside a lane follows *Depends*.
   A PR that fails its own *Verify* goes back to the same subagent once
   (`resume`), then is closed and re-queued with the next tier model.
4. **Wave 4 V1** runs only when every other PR is merged.

### Subagent prompt template

```
You are a cda-dev worker for chatondearu/ai-dev-kit, task <Id> of plans/PLAN-90-kiss-audit-2026-11.md (child issue #<n>, epic #90).
KISS full applies. Read AGENTS.md, the plan row for <Id>, PRD-90, and the ADRs in the plan's Constraints table before editing.

Setup: from the repo root run
  git fetch origin && git worktree add .orch/worktrees/<Id>-issue-<n> -b fix/<n>-<slug> origin/main
and work only inside that worktree. Never run install.sh against the real user config (override HOME, CLAUDE_CONFIG_DIR, CURSOR_HOME, XDG_CONFIG_HOME with temp dirs). Run commands through `nix develop -c` when a flake is present.

Contract: <Task cell>. You own only: <Owns cell>. Do not edit other files; if you must, stop and report why.
Every non-trivial branch you add gets one test. Keep existing error handling.

Before opening the PR: run <Verify cell> and the AGENTS.md "Verify (run before every PR)" block; rebase on origin/main; conventional commit messages; code comments in English.
Open the PR with `gh pr create --base main --title "<type>(<area>): <short>" --body` containing: Closes #<n>, Part of #90, what changed, how you verified (paste the command output tail), and a `worker-result` JSON (status, checks, pr) per skills/core/cda-dev/templates/worker-result.schema.json.
Do not merge. Final answer: PR URL + the worker-result JSON. If blocked, say exactly what is missing.
```

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Two tasks in one lane touch the same file (`orch.sh`, `graph.sh`, `tui.rs`) | *Owns* column + *Depends* ordering; rebase before PR; H3/H4 own disjoint regions and are reviewed together |
| H2 makes legitimate Ready issues `blocked` on repos without a verify command | `orch: allow-no-verify` escape hatch; documented in conventions and ADR-006 |
| Reviewer agent rubber-stamps | reviewer.md checklist gets observable predicates (S5) and runs in a fresh context with the diff only; V1 re-checks on a seeded lazy diff |
| C4 removes a path the tailnet server depends on | `poll_sessions` is explicitly kept; `cargo test` on `cdamuxd` + manual `GET /v1/sessions` |
| C5 regression in state classification | existing `tmux::tests::classify_*` + a new parser test for the `list-panes -a` format |
| A fast-tier model under-delivers | one `resume` then escalate one tier; never more than two attempts per task |
| Subagents launched from Cursor do not see KISS hooks (finding 15) | the template inlines the KISS contract and the Verify requirement |

## Test plan

Seams under test (public boundaries, not internals):

- `orch_resolve_verify(repo_dir, issue_body)` → command string or `blocked` reason
- `orch_render_prompt` output for a known issue → contains PRD path and ADR list
- graph-plan fan-out → number of concurrent workers ≤ `max_parallel`; diff outside `paths` → blocked
- `orch.sh` process group on `INT` → no surviving children; `orch-auto.sh` exit code when locked
- cdamux CLI: `cdamux send --enter` on a throwaway tmux server; `session_name_for_path`; `tmux_has_session`; `apply_pane_split` with `base-index 1`
- tailnet HTTP: bind resolution, `?repo=` traversal, token header
- Fleet tick: tmux spawn count for N sessions

- [ ] Each seam has one test in the PR that touches it
- [ ] AGENTS.md Verify block green on `main` after each wave
- [ ] `cargo test` + `cargo build` 0 warnings after wave 3

## Verification

- [ ] Lint / test / build per project conventions (AGENTS.md Verify block, `cargo test`)
- [ ] ADR-006 recorded (review gate + fail-closed verify); ADR-004 amended (tailnet token)
- [ ] PRD-90 → shipped, this plan → done, `import-kanban.sh` run, `plans/README.md` + `foam/index.md` updated
- [ ] `doc/` synced for changed operator behavior (`orch: allow-no-verify`, reviewer step, tailnet token, Cursor rules note)
