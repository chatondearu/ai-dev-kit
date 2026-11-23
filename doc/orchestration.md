# Orchestration — how to run

Skill install path (after `./install.sh`):

```bash
SKILL=~/.cursor/skills/cda-dev   # or Claude / opencode skills dir
```

Host readiness (checks only, never installs):

```bash
./scripts/doctor.sh
```

## Interactive orchestrator (`cdamux orch new`)

`cdamux orch new [--agent X]` starts one long-lived session you chat with; it
follows skill `cda-orchestrator`: it never edits or verifies itself. Chat
requests get one question round + GO, then a planner worker (or fast-lane brief)
before dispatch. Kanban issues go to an `orch.sh` sub-run (exit 75 → report
`lock held by run <id>`, never spawn duplicates); research and small fixes to
interactive workers; results via `cdamux msg`. After dispatch it ends the turn
and waits for a nudge. The boot prompt is delivered via
`$CDAMUX_HOME/prompts/<host>.md` on the client command line (same path as
workers); the pane exports `CDA_ROLE=orchestrator` next to `CDAMUX_AGENT_ID`.
Requires a `cdamux` with `orch`, `worker`, `msg` (Nix: `cda.aitools.cdamux`).

`cdamux orch new --global` starts (or switches to) the single live
cross-project orchestrator: cwd `$CDAMUX_HOME/global`, agent `scope=global`,
global boot prompt (list projects, pick a repo, dispatch with
`worker spawn --repo` / `orch.sh --repo`). Fleet shows it under
“— global orchestrator”; the spawn modal offers the same via **o** then **g**.

`cdamux orch new --child --path <path|project>` nests a project orchestrator
under the caller’s global session (`$CDAMUX_AGENT_ID` must be `scope=global`):
fresh run, project boot prompt, `parent` = caller, no `scope=global`
(ADR-015 depth ≤ 2). `--path` and `--child` are incompatible with `--global`.

`cdamux orch ls [--json]` lists every recorded orchestrator (global, nested,
stale) with `scope`, `live`, `repo` and `parent`; `cdamux capture <session>`
prints a session's pane text read-only. Details:
[cdamux architecture](../cdamux/docs/architecture.md#read-only-inspection-388).

## Graph orchestration (`cda-graph`)

See **[graph-orchestration.md](graph-orchestration.md)** (cookbook, env vars, graph-plan, cdamux).
Skill: `cda-graph`. ADR: [ADR-005](../foam/decisions/ADR-005-cda-graph-orchestration.md).
Orca comparison: [orca-mapping.md](orca-mapping.md).

## Manual ready-pickup

```bash
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --dry-run
bash "$SKILL/scripts/orch.sh" --mode pr-fix --pr 88
```

Requires `gh`, an agent CLI, and (for fast-lane) an agent brief on the
issue when no `plans/PLAN-<id>-*` exists. Only brief comments by an
`OWNER`/`MEMBER`/`COLLABORATOR` count; anyone else's brief is ignored.

Each role (`worker`, `reviewer`, `verifier`, `integrator`, `pr-fix`) runs the CLI
resolved as `ORCH_AGENT_<ROLE>` > `ORCH_AGENT` / `--agent` > the CLI of the session
that launched `orch.sh` > first CLI on `PATH`
([ADR-010](../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers.md)). `--dry-run`
prints one `agent[<role>]: <cli> (<source>)` line per role, source being
`role-override`, `override`, `parent` or `path`:

```bash
ORCH_AGENT_VERIFIER=codex bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --dry-run
```

Each role also runs at a **tier** (`small`, `medium`, `large`; `integrator`
defaults to `small`, the others to `medium`), turned into CLI flags by
[`config/tiers.tsv`](../skills/core/cda-dev/config/tiers.tsv) — e.g. `claude` at
`medium` runs `claude -p --model sonnet --effort high`. `ORCH_TIER_<ROLE>` and
`ORCH_TIERS_FILE` change it ([configuration](configuration.md)); an agent command that
already passes `--model` / `-m` is `pinned`. `--dry-run` adds a
`tier[<role>]: <tier> <flags>` line per role.

**Escalation**: in the worker and `pr-fix` loops each failed iteration (red verify or failed
grounding verifier) re-runs one tier up (`small` → `medium` → `large`), bounded by
`ORCH_TIER_MAX` (default `large`). A failure at the ceiling ends the worker `blocked`
with `tier ceiling <tier> reached: <failure>`; a pinned command only stops at
`ORCH_MAX_ITER`. Worker Agents and graph nodes under `$CDAMUX_HOME` show the tier
of the latest run.

`pr-fix` only works on same-repo PRs: a cross-repository (fork) PR, a PR whose
head branch is the default branch, or unreadable PR metadata ends `blocked`
before any agent run (and before the PR is checked out).

`pr-fix` also acts on **review feedback**. `orch.sh` fetches, with `gh`, the latest
`CHANGES_REQUESTED` review per author, unresolved review threads (first 100) and
conversation comments newer than the head commit, keeps only `OWNER`/`MEMBER`/`COLLABORATOR`
authors, and appends them, quoted, to the prompt as `## Review feedback`; the agent never
fetches PR text itself. After fixing, the worker posts one summary comment (starting with
`<!-- orch:pr-fix-summary -->`, which `orch` ignores) mapping each item to its commit or the
reason it was not addressed; it never resolves threads. An item that conflicts with the plan,
an ADR or a product decision ends `blocked`. A failed feedback lookup ends `blocked`; no
trusted feedback and green checks (or no checks with `allow-no-checks`) ends `done: PR N
nothing to fix` with no agent run and no checkout.

Per issue the harness runs: worker loop (agent → verify, retry prompt carries
the failed verify output or verifier findings) → PR exists → **independent review** (fresh
`reviewer.md` agent at tier `large`, verdict read from a file in a `mktemp -d` dir
outside the worktree and `.orch/`, never from stdout;
`changes_requested` gives the worker one fix iteration, then re-review once, then
`blocked`) → CI green + `MERGEABLE` → `done`. `pr-fix` runs the same review gate.

Ready-pickup fetches the repo's default branch (resolved from `origin`, never
assumed `main`) and cuts every worktree from `origin/<default>`, so a plan merged
upstream is found even when the local checkout is stale. An agent that exits
non-zero fails its iteration (no verify on an untouched tree). An ADR-016 merge
holds `.orch/merge.lock` (sibling workers merge one at a time), fetches again,
rebases + re-runs verify + pushes when the branch is behind, refuses
`mergeable=UNKNOWN`, and merges with `gh pr merge --rebase` only; a repo that
disallows rebase merges, or any failed step, ends the issue `error`. The lock is
`flock(2)` (flock(1), else a perl helper on stock macOS), so a dead holder never
leaves it stale; merges by hand use `skills/core/cda-dev/scripts/merge-lock.sh --repo <repo> merge <N>`
(same lock, fetch, behind → refused, `--rebase --match-head-commit`). Every `gh` call of
the merge path runs in the worktree (or `--repo`), never an inherited `GH_REPO`.
Before each graph iteration, before PR lookup, and before each review round it
checks the branch PR state (`gh pr list --state all`): **MERGED** → `done`
(`merged by maintainer`), no further pushes. Lookup accepts OPEN or MERGED.
If the loop is green but no PR exists, one bounded agent step asks to commit,
push, and open a PR with `Closes #N`, then re-looks up; still none → `blocked`.
At run end `orch-cleanup.sh` removes the worktrees of `done` issues; `blocked` ones stay.
See [ADR-006](../foam/decisions/ADR-006-review-gate-fail-closed-verify.md).

**Verify is fail-closed.** `orch` prefers the fenced block under the `Verify`
heading of `AGENTS.md` over a `verify:` line. When no verify command can be
found at all, the issue ends `blocked` with
`verify: no command (add 'orch: allow-no-verify' to the issue body to skip)`.
Put the literal `orch: allow-no-verify` in the issue body to accept `true` for
that issue (e.g. docs-only repos), or set `ORCH_VERIFY` to a command. The marker
is ignored when the issue author is not `OWNER`/`MEMBER`/`COLLABORATOR`.

### Watch progress (agents / humans)

While orch runs, poll machine-readable status (updated on every phase change;
CI heartbeats ~30s):

```bash
ls "$CDAMUX_HOME/runs"             # orch runs (Fleet / API source of truth)
cat .orch/run/<issue>.json         # one worker
tail -f .orch/orch-<issue>.log     # if you tee'd a log
```

Each worker entry also has `phases`: one `{phase, started_at}` (epoch seconds) per
phase change, oldest first; `pending` resets it. cdamux builds its step bar and
durations from it. `cli`, `model`, `effort` and `tier` record what the worker's
own agent (not its reviewer / verifier) last ran; `effort` is absent when the CLI takes
none, and `pending` clears them for a new run.

Empty `gh pr checks` without `ORCH_ALLOW_NO_CHECKS=1` / `orch: allow-no-checks`
exits immediately with status `none` (does not wait for CI that will never start).

## AFK poller (`orch-auto`)

Discovers Ready cards from `foam/board/` (`cda-kanban` `foam-board.sh
list-ready`; no GitHub Projects). Skips `triage:ready-for-human` and issues
that already have an open PR on `feat/<id>-issue-<id>` when the card has
`github_issue`. Then runs `orch.sh --mode ready-pickup`. If a label or PR
lookup fails (rate limit, network), that GitHub-backed card is skipped (fail
closed). Foam-only slugs skip those GitHub lookups.

```bash
bash "$SKILL/scripts/orch-auto.sh" --repo /path/to/repo --dry-run
bash "$SKILL/scripts/orch-auto.sh" --repo /path/to/repo
```

Board listing uses `cda-kanban` `foam-board.sh list-ready`. Override path with
`ORCH_FOAM_BOARD` (or legacy `ORCH_GH_BOARD`) if needed (tests / custom layout).

One run at a time: lock on `<repo>/.orch/auto.lock` (`flock`, else the dir `auto.lock.d/`). Logs under
`<repo>/.orch/auto/` (newest 50 kept). A second run exits 75 (EX_TEMPFAIL) when the lock is held, so a timer sees it did not run.

### Systemd (opt-in)

Templates: `skills/core/cda-dev/templates/systemd/`. **Not** enabled by
`install.sh`.

```bash
mkdir -p ~/.config/systemd/user
cp "$SKILL/templates/systemd/cda-orch-auto."{service,timer} ~/.config/systemd/user/
cp "$SKILL/templates/systemd/cda-orch-auto.env.example" ~/.config/cda-orch-auto.env
# Edit ORCH_REPO, ORCH_AGENT (required: orch-auto exits 2 without it) and PATH
systemctl --user daemon-reload
systemctl --user enable --now cda-orch-auto.timer   # AFK ON
systemctl --user stop cda-orch-auto.timer            # free this machine
```

Prefer a **dedicated host** (clone + `gh auth` + agent CLI) when the laptop
should stay free for interactive work.

## Worker function export

Sandbox and `bash -c` workers need harness functions (`orch_*` / `_orch_*`) via
`export -f`. `orch_export_functions` discovers them with `declare -F` only (not
`compgen`, which some agent shells lack) and asserts that a child bash can
`type orch_loop_worker` before invoke. If that assert still fails, use the
skill-only path or `cda-graph` [`graph-node.sh`](graph-orchestration.md) with a
Cursor Task instead of relying on the harness `bash -c` worker.

## Headless (no Kanban)

```bash
bash "$SKILL/scripts/headless.sh" --prompt "fix the flaky auth test" --dry-run
bash "$SKILL/scripts/headless.sh" --prompt-file ./task.md --max-iter 5
```

## Triage handoff

1. Groom with skill `cda-triage` until `ready-for-agent` + Status Ready + brief.
2. Either run orch manually, or enable orch-auto / the timer.
3. Review the PR; merge yourself.
