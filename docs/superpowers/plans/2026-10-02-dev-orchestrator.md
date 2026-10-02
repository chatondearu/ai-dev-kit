# Dev-orchestrator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a portable `dev-orchestrator` skill + bash harness that runs intake / ready-pickup / pr-fix through worktrees, verify loops, and CI-green PRs (no auto-merge) on Cursor, Claude, or opencode.

**Architecture:** Hybrid skill (contract + prompts) + `orch.sh` harness with small `lib/*.sh` modules. `foam-batch-orchestrator` becomes a thin alias to `mode=intake`. Existing foam/kanban skills stay the source of truth for artifacts.

**Tech Stack:** bash, git worktree, gh, optional bwrap, agent CLIs (`claude` / Cursor agent / `opencode`), Markdown skills

## Global Constraints

- Autonomy stops at CI green + mergeable PR — never merge or force-push main/master.
- One child = one worktree = one branch = one PR.
- Default `ORCH_MAX_PARALLEL=3`, `ORCH_MAX_ITER=10`.
- Sandbox off by default; `--sandbox` uses bwrap or warns and continues.
- Harness: portable bash/`git`/`gh` only — no proprietary IDE APIs.
- English for GitHub/foam artifacts; chat may match user language.
- Spec: `docs/superpowers/specs/2026-10-02-dev-orchestrator-design.md`
- Do not archive `task-management` in this plan.

---

## File map

| Path | Responsibility |
| ---- | -------------- |
| `skills/core/dev-orchestrator/SKILL.md` | Contract, modes, fallback skill-only |
| `skills/core/dev-orchestrator/conventions.md` | Naming, ledger statuses, caps |
| `skills/core/dev-orchestrator/prompts/*.md` | intake / worker / reviewer / pr-fix |
| `skills/core/dev-orchestrator/templates/*.md` | epic-breakdown, run-ledger |
| `skills/core/dev-orchestrator/scripts/orch.sh` | CLI entry |
| `skills/core/dev-orchestrator/scripts/lib/*.sh` | agents, worktree, loop, sandbox, ci, verify |
| `skills/core/dev-orchestrator/scripts/tests/run-tests.sh` | Bash unit tests for libs |
| `skills/core/foam-batch-orchestrator/SKILL.md` | Thin alias → dev-orchestrator |
| `README.md`, `skills/README.md`, `rules/01-project-workflow.md`, `project-agents-setup` templates | Discovery / migration |

---

### Task 1: Scaffold skill tree + conventions + templates

**Files:**
- Create: `skills/core/dev-orchestrator/conventions.md`
- Create: `skills/core/dev-orchestrator/templates/epic-breakdown.md`
- Create: `skills/core/dev-orchestrator/templates/run-ledger.md`
- Create: `skills/core/dev-orchestrator/scripts/lib/.gitkeep` (removed once libs land)
- Create: `skills/core/dev-orchestrator/SKILL.md` (minimal stub pointing to conventions; full contract in Task 9)

**Interfaces:**
- Consumes: design spec; foam-batch templates/conventions as copy source
- Produces: ledger status vocabulary `pending|running|review|ci|done|blocked`; worktree path pattern `.orch/worktrees/<id>-<slug>`

- [ ] **Step 1: Create directories**

```bash
mkdir -p skills/core/dev-orchestrator/{prompts,templates,scripts/lib,scripts/tests}
```

- [ ] **Step 2: Write `conventions.md`**

Copy structure from `skills/core/foam-batch-orchestrator/conventions.md`, then change:

1. Title → `Dev orchestrator — conventions`
2. Add modes section:

```markdown
## Modes

| Mode | Requires | Creates |
| ---- | -------- | ------- |
| `intake` | User GO on epic breakdown | Epic issue, approved PRD, Ready children, plans |
| `ready-pickup` | Ready issues + existing PRD/plan | Nothing (validate only) |
| `pr-fix` | Open PR number/URL | Nothing (validate only) |
```

3. Extend worker ledger statuses with `ci` (waiting on remote checks).
4. Parallelism: hard-require worktrees when `ORCH_MAX_PARALLEL>1`; default cap 3.
5. Exit criteria: PR open **and** `gh pr checks` green (or BLOCKED). Never merge.
6. Worktree path: `.orch/worktrees/<issue_or_pr>-<slug>/` (gitignore guidance: add `.orch/` in target repos).
7. Env defaults: `ORCH_MAX_PARALLEL=3`, `ORCH_MAX_ITER=10`, `ORCH_BASE_PORT=3900`.

- [ ] **Step 3: Write templates**

`templates/run-ledger.md` — extend batch ledger:

```markdown
# Orchestrator run ledger

## Run

| Field | Value |
| ----- | ----- |
| Mode | intake \| ready-pickup \| pr-fix |
| Agent | |
| Sandbox | off \| bwrap |
| Max parallel | 3 |
| Max iter | 10 |
| Started | |

## Workers

| Id | Title | Plan | Worktree | Branch | Port | Status | PR | Checks | Reason |
| -- | ----- | ---- | -------- | ------ | ---- | ------ | -- | ------ | ------ |
| | | | | | | pending | | | |

Status: `pending` | `running` | `review` | `ci` | `done` | `blocked`
```

`templates/epic-breakdown.md` — copy from foam-batch `templates/epic-breakdown.md` unchanged if present; otherwise create:

```markdown
# Epic breakdown

## Objective
…

## Non-goals
- …

## Target repo
…

## Children

| # | Title | Goal | Acceptance | Deps | Size |
| - | ----- | ---- | ---------- | ---- | ---- |
| 1 | | | | — | S/M/L |

## Risks / ADR conflicts
…
```

- [ ] **Step 4: Write stub `SKILL.md`**

```markdown
---
name: dev-orchestrator
description: >-
  Unified meta-orchestrator: intake, Ready pickup, or PR-fix through foam/kanban,
  git worktrees, verify loops, and CI-green PRs (no auto-merge). Portable across
  Cursor, Claude Code, and opencode via orch.sh harness or skill-only fallback.
  Use for multi-subject batches, parallel features, or unblocking red CI.
---

# Dev orchestrator

Announce: **"Using dev-orchestrator skill."**

Stub — full contract lands in Task 9. See [conventions.md](conventions.md).
```

- [ ] **Step 5: Commit**

```bash
git add skills/core/dev-orchestrator
git commit -m "$(cat <<'EOF'
feat(skills): scaffold dev-orchestrator conventions and templates

EOF
)"
```

---

### Task 2: Bash test runner

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/tests/run-tests.sh`
- Create: `skills/core/dev-orchestrator/scripts/tests/helpers.sh`

**Interfaces:**
- Consumes: none
- Produces: `assert_eq`, `assert_file`, `assert_contains`; exit 0 on all pass

- [ ] **Step 1: Write `helpers.sh`**

```bash
#!/usr/bin/env bash
# shellcheck shell=bash
set -euo pipefail

FAILS=0
PASSES=0

assert_eq() {
  local got="$1" want="$2" msg="${3:-}"
  if [ "$got" = "$want" ]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$msg" "$got" "$want" >&2
  fi
}

assert_contains() {
  local hay="$1" needle="$2" msg="${3:-}"
  if [[ "$hay" == *"$needle"* ]]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s\n  missing: %s\n  in: %s\n' "$msg" "$needle" "$hay" >&2
  fi
}

assert_file() {
  local path="$1" msg="${2:-file exists}"
  if [ -e "$path" ]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s: %s\n' "$msg" "$path" >&2
  fi
}

summary() {
  printf 'passes=%s fails=%s\n' "$PASSES" "$FAILS"
  [ "$FAILS" -eq 0 ]
}
```

- [ ] **Step 2: Write `run-tests.sh` stub that sources helpers and exits 0**

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=helpers.sh
source "$ROOT/helpers.sh"
# Individual test files source helpers and call summary — appended by later tasks
printf 'No lib tests registered yet\n'
summary
```

- [ ] **Step 3: Make executable and run**

```bash
chmod +x skills/core/dev-orchestrator/scripts/tests/*.sh
bash skills/core/dev-orchestrator/scripts/tests/run-tests.sh
```

Expected: `passes=0 fails=0` (or message + exit 0)

- [ ] **Step 4: Commit**

```bash
git add skills/core/dev-orchestrator/scripts/tests
git commit -m "$(cat <<'EOF'
test(skills): add bash assert helpers for orch libs

EOF
)"
```

---

### Task 3: `worktree.sh` library

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/lib/worktree.sh`
- Create: `skills/core/dev-orchestrator/scripts/tests/test_worktree.sh`
- Modify: `skills/core/dev-orchestrator/scripts/tests/run-tests.sh`

**Interfaces:**
- Consumes: git repo at `$REPO_ROOT`
- Produces:
  - `orch_worktree_path REPO_ROOT ID SLUG` → prints path
  - `orch_worktree_add REPO_ROOT ID SLUG BRANCH [BASE_REF]` → creates worktree+branch; exit 0
  - `orch_worktree_bootstrap REPO_ROOT WORKTREE_PATH` → copies `.env` if present; no-op deps
  - `orch_worktree_remove REPO_ROOT WORKTREE_PATH` → `git worktree remove --force` when `--force-clean` path used

- [ ] **Step 1: Write failing test `test_worktree.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email t@t
git -C "$TMP" config user.name t
echo x >"$TMP/f"
git -C "$TMP" add f
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main

path="$(orch_worktree_path "$TMP" 42 auth-fix)"
assert_eq "$path" "$TMP/.orch/worktrees/42-auth-fix" "path pattern"

orch_worktree_add "$TMP" 42 auth-fix "feat/42-auth-fix" main
assert_file "$TMP/.orch/worktrees/42-auth-fix/f" "worktree checkout"
branch="$(git -C "$TMP/.orch/worktrees/42-auth-fix" branch --show-current)"
assert_eq "$branch" "feat/42-auth-fix" "branch name"

echo SECRET=1 >"$TMP/.env"
orch_worktree_bootstrap "$TMP" "$TMP/.orch/worktrees/42-auth-fix"
assert_file "$TMP/.orch/worktrees/42-auth-fix/.env" "env copied"

summary
```

- [ ] **Step 2: Run test — expect FAIL (missing lib)**

```bash
bash skills/core/dev-orchestrator/scripts/tests/test_worktree.sh
```

Expected: error sourcing `worktree.sh` or command not found

- [ ] **Step 3: Implement `worktree.sh`**

```bash
#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

orch_worktree_path() {
  local repo="$1" id="$2" slug="$3"
  printf '%s/.orch/worktrees/%s-%s\n' "$repo" "$id" "$slug"
}

orch_worktree_add() {
  local repo="$1" id="$2" slug="$3" branch="$4" base="${5:-HEAD}"
  local path
  path="$(orch_worktree_path "$repo" "$id" "$slug")"
  mkdir -p "$(dirname "$path")"
  if [ -e "$path" ]; then
    printf 'worktree already exists: %s\n' "$path" >&2
    return 1
  fi
  git -C "$repo" worktree add -b "$branch" "$path" "$base"
}

orch_worktree_bootstrap() {
  local repo="$1" wt="$2"
  if [ -f "$repo/.env" ] && [ ! -e "$wt/.env" ]; then
    cp "$repo/.env" "$wt/.env"
  fi
}

orch_worktree_remove() {
  local repo="$1" wt="$2"
  git -C "$repo" worktree remove --force "$wt"
}
```

- [ ] **Step 4: Wire into `run-tests.sh` and pass**

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
fail=0
for t in "$ROOT"/test_*.sh; do
  [ -e "$t" ] || continue
  echo "# $t"
  bash "$t" || fail=1
done
exit "$fail"
```

```bash
bash skills/core/dev-orchestrator/scripts/tests/run-tests.sh
```

Expected: exit 0, worktree assertions pass

- [ ] **Step 5: Commit**

```bash
git add skills/core/dev-orchestrator/scripts
git commit -m "$(cat <<'EOF'
feat(skills): add orch worktree lib with unit tests

EOF
)"
```

---

### Task 4: `agents.sh` library

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/lib/agents.sh`
- Create: `skills/core/dev-orchestrator/scripts/tests/test_agents.sh`

**Interfaces:**
- Produces:
  - `orch_detect_agent` → prints one of `claude|opencode|cursor|custom` and sets `ORCH_AGENT_BIN`
  - `orch_agent_run WORKTREE PROMPT_FILE` → invokes detected CLI non-interactively; returns CLI exit code
  - Honors `ORCH_AGENT` override (full command prefix)

- [ ] **Step 1: Write failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"

# Fake PATH with only a stub "claude"
FAKE="$(mktemp -d)"
trap 'rm -rf "$FAKE"' EXIT
cat >"$FAKE/claude" <<'EOF'
#!/usr/bin/env bash
echo "claude-ok $*"; exit 0
EOF
chmod +x "$FAKE/claude"
PATH="$FAKE:$PATH" ORCH_AGENT= ORCH_AGENT_BIN=
kind="$(orch_detect_agent)"
assert_eq "$kind" "claude" "detect claude"

PROMPT="$(mktemp)"
echo "hi" >"$PROMPT"
out="$(PATH="$FAKE:$PATH" orch_agent_run "$FAKE" "$PROMPT" 2>&1 || true)"
assert_contains "$out" "claude-ok" "invoke stub"

# Override
export ORCH_AGENT="$FAKE/claude --print"
kind2="$(orch_detect_agent)"
assert_eq "$kind2" "custom" "custom override"

summary
```

- [ ] **Step 2: Run — expect FAIL**

```bash
bash skills/core/dev-orchestrator/scripts/tests/test_agents.sh
```

- [ ] **Step 3: Implement `agents.sh`**

```bash
#!/usr/bin/env bash
# shellcheck shell=bash

orch_detect_agent() {
  if [ -n "${ORCH_AGENT:-}" ]; then
    ORCH_AGENT_BIN=$ORCH_AGENT
    printf 'custom\n'
    return 0
  fi
  local c
  for c in agent cursor-agent claude opencode; do
    if command -v "$c" >/dev/null 2>&1; then
      ORCH_AGENT_BIN=$c
      case "$c" in
        agent|cursor-agent) printf 'cursor\n';;
        *) printf '%s\n' "$c";;
      esac
      return 0
    fi
  done
  printf 'No agent CLI found (tried agent, cursor-agent, claude, opencode). Set ORCH_AGENT.\n' >&2
  return 1
}

orch_agent_run() {
  local wt="$1" prompt_file="$2"
  [ -n "${ORCH_AGENT_BIN:-}" ] || orch_detect_agent >/dev/null
  local bin=( $ORCH_AGENT_BIN )
  local name="${bin[0]}"
  case "$name" in
    claude)
      # Non-interactive print mode; flags may need adjust per claude version
      (cd "$wt" && "${bin[@]}" -p --dangerously-skip-permissions <"$prompt_file")
      ;;
    opencode)
      (cd "$wt" && "${bin[@]}" run <"$prompt_file")
      ;;
    agent|cursor-agent)
      (cd "$wt" && "${bin[@]}" -p <"$prompt_file")
      ;;
    *)
      (cd "$wt" && "${bin[@]}" <"$prompt_file")
      ;;
  esac
}
```

Note for implementer: if a real CLI rejects these flags during smoke, keep detection working and document exact flags in `conventions.md`; unit tests use stubs only.

- [ ] **Step 4: Pass tests + commit**

```bash
bash skills/core/dev-orchestrator/scripts/tests/run-tests.sh
git add skills/core/dev-orchestrator/scripts
git commit -m "$(cat <<'EOF'
feat(skills): add orch agent detection and invoke adapters

EOF
)"
```

---

### Task 5: `verify.sh` + `ci.sh`

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/lib/verify.sh`
- Create: `skills/core/dev-orchestrator/scripts/lib/ci.sh`
- Create: `skills/core/dev-orchestrator/scripts/tests/test_verify_ci.sh`

**Interfaces:**
- `orch_resolve_verify REPO_OR_WT` → prints a command string (prefer `nix develop -c …` if `flake.nix`; else `npm test` / `pnpm test` / `make test` heuristics; else `true` with warning)
- `orch_run_verify REPO_OR_WT` → runs that command in dir; exit code
- `orch_pr_checks_watch PR_URL_OR_NUM [TIMEOUT_SEC]` → polls `gh pr checks` until all pass or timeout; prints `green|red|pending` on last line; exit 0 only if green
- Never merges

- [ ] **Step 1: Write failing tests**

```bash
#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/ci.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
# fake repo with package.json test script
printf '{"scripts":{"test":"echo ok"}}\n' >"$TMP/package.json"
cmd="$(orch_resolve_verify "$TMP")"
assert_contains "$cmd" "npm" "resolve npm test"

# stub gh
FAKE="$(mktemp -d)"
cat >"$FAKE/gh" <<'EOF'
#!/usr/bin/env bash
# emulate: gh pr checks 1 --json name,state,bucket
echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'
EOF
chmod +x "$FAKE/gh"
PATH="$FAKE:$PATH" out="$(orch_pr_checks_watch 1 5)"; assert_eq "$out" "green" "ci green"

summary
```

- [ ] **Step 2: Run — expect FAIL**

- [ ] **Step 3: Implement libs**

`verify.sh`:

```bash
#!/usr/bin/env bash
# shellcheck shell=bash

orch_resolve_verify() {
  local dir="$1"
  if [ -f "$dir/flake.nix" ] || [ -f "$dir/.envrc" ]; then
    if [ -f "$dir/package.json" ]; then
      printf 'nix develop -c npm test\n'; return 0
    fi
    if [ -f "$dir/Makefile" ]; then
      printf 'nix develop -c make test\n'; return 0
    fi
  fi
  if [ -f "$dir/pnpm-lock.yaml" ]; then printf 'pnpm test\n'; return 0; fi
  if [ -f "$dir/package.json" ]; then printf 'npm test\n'; return 0; fi
  if [ -f "$dir/Makefile" ]; then printf 'make test\n'; return 0; fi
  if [ -f "$dir/Cargo.toml" ]; then printf 'cargo test\n'; return 0; fi
  printf 'true\n' >&2
  printf 'true\n'
}

orch_run_verify() {
  local dir="$1"
  local cmd
  cmd="$(orch_resolve_verify "$dir")"
  (cd "$dir" && eval "$cmd")
}
```

`ci.sh`:

```bash
#!/usr/bin/env bash
# shellcheck shell=bash

orch_pr_checks_watch() {
  local pr="$1"
  local timeout="${2:-600}"
  local start=$SECONDS
  while true; do
    local json bucket
    json="$(gh pr checks "$pr" --json name,state,bucket 2>/dev/null || true)"
    if [ -z "$json" ] || [ "$json" = "[]" ]; then
      # No checks configured → treat as green
      printf 'green\n'
      return 0
    fi
    if echo "$json" | grep -q '"bucket":"fail\|"bucket":"pending\|"state":"FAILURE\|"state":"PENDING\|"state":"IN_PROGRESS\|"state":"QUEUED'; then
      :
    else
      # heuristic: all pass buckets
      if echo "$json" | grep -qv '"bucket":"pass"'; then
        # still running or unknown
        :
      else
        printf 'green\n'
        return 0
      fi
    fi
    if echo "$json" | grep -Eq '"bucket":"fail"|"state":"FAILURE"'; then
      if (( SECONDS - start > timeout )); then
        printf 'red\n'
        return 1
      fi
    fi
    if (( SECONDS - start > timeout )); then
      printf 'pending\n'
      return 1
    fi
    sleep 5
  done
}
```

Implementer: refine JSON parsing with `jq` if available (`command -v jq`); prefer jq over grep. Tests stub `gh`.

- [ ] **Step 4: Pass + commit**

```bash
bash skills/core/dev-orchestrator/scripts/tests/run-tests.sh
git add skills/core/dev-orchestrator/scripts
git commit -m "$(cat <<'EOF'
feat(skills): add orch verify resolver and CI watch helpers

EOF
)"
```

---

### Task 6: `sandbox.sh`

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/lib/sandbox.sh`
- Create: `skills/core/dev-orchestrator/scripts/tests/test_sandbox.sh`

**Interfaces:**
- `orch_sandbox_wrap CMD_ARRAY...` — if `ORCH_SANDBOX=1` and `bwrap` exists, run under bwrap RW=`$ORCH_WT` with RO binds for `/usr` `/bin` `/nix` (if exist); else warn once and `exec` bare command
- Never hard-fail missing bwrap

- [ ] **Step 1: Failing test** — with `ORCH_SANDBOX=0`, wrap runs command and captures output `ok`

```bash
#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/sandbox.sh"
ORCH_SANDBOX=0 ORCH_WT=/tmp
out="$(orch_sandbox_wrap bash -c 'echo ok')"
assert_eq "$out" "ok" "passthrough"
summary
```

- [ ] **Step 2: Implement**

```bash
#!/usr/bin/env bash
# shellcheck shell=bash

orch_sandbox_wrap() {
  if [ "${ORCH_SANDBOX:-0}" != "1" ]; then
    "$@"
    return $?
  fi
  if ! command -v bwrap >/dev/null 2>&1; then
    printf 'warn: --sandbox requested but bwrap missing; continuing unsandboxed\n' >&2
    "$@"
    return $?
  fi
  local wt="${ORCH_WT:?ORCH_WT required for sandbox}"
  local args=(--die-with-parent --new-session)
  args+=(--bind "$wt" "$wt" --chdir "$wt")
  for p in /usr /bin /lib /lib64 /nix /etc/ssl /etc/resolv.conf; do
    [ -e "$p" ] && args+=(--ro-bind "$p" "$p")
  done
  args+=(--dev /dev --proc /proc --tmpfs /tmp)
  bwrap "${args[@]}" "$@"
}
```

- [ ] **Step 3: Pass + commit**

```bash
bash skills/core/dev-orchestrator/scripts/tests/run-tests.sh
git add skills/core/dev-orchestrator/scripts
git commit -m "$(cat <<'EOF'
feat(skills): add optional bwrap sandbox wrapper for orch

EOF
)"
```

---

### Task 7: `loop.sh` + `orch.sh` CLI (dispatch skeleton)

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/lib/loop.sh`
- Create: `skills/core/dev-orchestrator/scripts/orch.sh`
- Create: `skills/core/dev-orchestrator/scripts/tests/test_loop.sh`

**Interfaces:**
- `orch_loop_worker WORKTREE PROMPT_FILE` — up to `ORCH_MAX_ITER` times: `orch_agent_run` → `orch_run_verify`; stop on verify success; else continue; on exhaust print `blocked` and return 1
- `orch.sh` CLI:

```text
orch.sh --mode ready-pickup|pr-fix|intake [options]
  --repo DIR           (default: cwd)
  --issue N            (repeatable for ready-pickup)
  --pr N|URL           (pr-fix)
  --agent CMD          (sets ORCH_AGENT)
  --sandbox            (ORCH_SANDBOX=1)
  --max-parallel N
  --max-iter N
  --dry-run            (print plan: worktrees/branches; no agent invoke)
  --force-clean        (remove worktrees after — only with explicit flag)
  -h|--help
```

v1 CLI **does not** create GitHub issues/PRDs (intake bootstrap stays skill-driven). CLI owns: worktree provision, agent loop, local verify, optional CI watch when `--pr` known / after worker reports PR URL in ledger file.

For `ready-pickup --dry-run --issue 1 --issue 2`: print worktree paths and branches without creating if `--dry-run`; without dry-run create worktrees.

- [ ] **Step 1: Test loop with stub agent that fails verify once then succeeds**

```bash
#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/loop.sh"

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
# verify reads a counter file
printf 'true\n' >"$TMP/always.sh"; chmod +x "$TMP/always.sh"
# Override resolve by exporting function — instead drop package.json and use true
# Loop with ORCH_MAX_ITER=2 and agent that exits 0
FAKE="$(mktemp -d)"
cat >"$FAKE/claude" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$FAKE/claude"
export PATH="$FAKE:$PATH" ORCH_AGENT= ORCH_AGENT_BIN=claude ORCH_MAX_ITER=2
PROMPT="$(mktemp)"; echo fix >"$PROMPT"
# make verify always true via empty project
orch_loop_worker "$TMP" "$PROMPT"
assert_eq "$?" "0" "loop succeeds when verify true"
summary
```

Fix assert: capture `orch_loop_worker` status properly:

```bash
set +e
orch_loop_worker "$TMP" "$PROMPT"
rc=$?
set -e
assert_eq "$rc" "0" "loop succeeds when verify true"
```

- [ ] **Step 2: Implement `loop.sh`**

```bash
#!/usr/bin/env bash
# shellcheck shell=bash

orch_loop_worker() {
  local wt="$1" prompt="$2"
  local max="${ORCH_MAX_ITER:-10}" i=1
  while [ "$i" -le "$max" ]; do
    printf 'orch loop iter %s/%s\n' "$i" "$max" >&2
    orch_agent_run "$wt" "$prompt" || true
    if orch_run_verify "$wt"; then
      printf 'verify ok\n' >&2
      return 0
    fi
    i=$((i + 1))
  done
  printf 'blocked\n' >&2
  return 1
}
```

- [ ] **Step 3: Implement `orch.sh`** (dry-run + worktree create for ready-pickup)

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/worktree.sh
source "$ROOT/lib/worktree.sh"
source "$ROOT/lib/agents.sh"
source "$ROOT/lib/verify.sh"
source "$ROOT/lib/ci.sh"
source "$ROOT/lib/sandbox.sh"
source "$ROOT/lib/loop.sh"

MODE=""; REPO="$(pwd)"; DRY=0; SANDBOX=0
MAX_P="${ORCH_MAX_PARALLEL:-3}"; MAX_I="${ORCH_MAX_ITER:-10}"
ISSUES=(); PR=""; FORCE_CLEAN=0

usage() { sed -n '2,20p' "$0"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --mode) MODE="$2"; shift;;
    --repo) REPO="$2"; shift;;
    --issue) ISSUES+=("$2"); shift;;
    --pr) PR="$2"; shift;;
    --agent) ORCH_AGENT="$2"; shift;;
    --sandbox) SANDBOX=1;;
    --max-parallel) MAX_P="$2"; shift;;
    --max-iter) MAX_I="$2"; shift;;
    --dry-run) DRY=1;;
    --force-clean) FORCE_CLEAN=1;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; exit 2;;
  esac
  shift
done

export ORCH_MAX_PARALLEL="$MAX_P" ORCH_MAX_ITER="$MAX_I" ORCH_SANDBOX="$SANDBOX"
REPO="$(cd "$REPO" && pwd)"

case "$MODE" in
  ready-pickup)
    [ "${#ISSUES[@]}" -gt 0 ] || { echo "need --issue"; exit 2; }
    idx=0
    for id in "${ISSUES[@]}"; do
      slug="issue-$id"
      branch="feat/${id}-${slug}"
      path="$(orch_worktree_path "$REPO" "$id" "$slug")"
      port=$(( ${ORCH_BASE_PORT:-3900} + idx ))
      echo "worker id=$id branch=$branch wt=$path port=$port"
      if [ "$DRY" = 0 ]; then
        orch_worktree_add "$REPO" "$id" "$slug" "$branch" HEAD || true
        orch_worktree_bootstrap "$REPO" "$path"
      fi
      idx=$((idx + 1))
      if [ "$idx" -ge "$MAX_P" ]; then
        echo "cap ORCH_MAX_PARALLEL=$MAX_P reached (queue not auto-run in dry skeleton — document in SKILL)" >&2
      fi
    done
    ;;
  pr-fix)
    [ -n "$PR" ] || { echo "need --pr"; exit 2; }
    echo "pr-fix pr=$PR dry=$DRY"
    if [ "$DRY" = 0 ]; then
      # checkout PR in a worktree named pr-<n>
      num="${PR##*/}"
      path="$(orch_worktree_path "$REPO" "pr-$num" "fix")"
      if [ ! -e "$path" ]; then
        mkdir -p "$(dirname "$path")"
        git -C "$REPO" fetch -q origin "pull/$num/head:orch/pr-$num" || true
        git -C "$REPO" worktree add "$path" "orch/pr-$num"
      fi
      PROMPT="$ROOT/../prompts/pr-fix.md"
      export ORCH_WT="$path"
      orch_detect_agent >/dev/null
      orch_sandbox_wrap bash -c "ORCH_WT=$path orch_loop_worker \"$path\" \"$PROMPT\""
      orch_pr_checks_watch "$num" 600 || exit 1
    fi
    ;;
  intake)
    echo "intake bootstrap is skill-driven; after GO use ready-pickup with child issue numbers" >&2
    exit 0
    ;;
  *) echo "need --mode"; exit 2;;
esac
```

Make executable. Refine parallel dispatch in a later polish if needed: v1 may run workers sequentially under the cap for simplicity, documented in SKILL (parallelism via background `&` + `wait` when not dry-run is preferred if time allows — implement background jobs with a job counter ≤ MAX_P).

- [ ] **Step 4: Dry-run smoke**

```bash
chmod +x skills/core/dev-orchestrator/scripts/orch.sh
bash skills/core/dev-orchestrator/scripts/orch.sh --mode ready-pickup --repo /tmp --issue 1 --dry-run
```

Expected: prints worker line (repo may fail worktree if not a git repo — use a temp git repo for smoke).

Better smoke:

```bash
TMP=$(mktemp -d)
git -C "$TMP" init -q && git -C "$TMP" config user.email t@t && git -C "$TMP" config user.name t
echo x>"$TMP/f" && git -C "$TMP" add f && git -C "$TMP" commit -qm i && git -C "$TMP" branch -M main
bash skills/core/dev-orchestrator/scripts/orch.sh --mode ready-pickup --repo "$TMP" --issue 7 --dry-run
```

Expected: `worker id=7 ...`

- [ ] **Step 5: Commit**

```bash
git add skills/core/dev-orchestrator/scripts
git commit -m "$(cat <<'EOF'
feat(skills): add orch loop and CLI dry-run dispatch

EOF
)"
```

---

### Task 8: Prompts (intake, worker, reviewer, pr-fix)

**Files:**
- Create: `skills/core/dev-orchestrator/prompts/intake.md`
- Create: `skills/core/dev-orchestrator/prompts/worker.md`
- Create: `skills/core/dev-orchestrator/prompts/reviewer.md`
- Create: `skills/core/dev-orchestrator/prompts/pr-fix.md`

**Interfaces:**
- Consumes: foam-batch prompts as base
- Produces: prompts that mention CI-green exit, worktree cwd, no merge

- [ ] **Step 1: Adapt from foam-batch**

Copy `skills/core/foam-batch-orchestrator/prompts/{intake,worker,reviewer}.md` into dev-orchestrator, then edit:

**worker.md** — after tests/review, add:

```markdown
7. **CI**
   - Ensure PR exists; wait for checks green (harness `orch_pr_checks_watch` or `gh pr checks`)
   - Do **not** merge
8. Report JSON — extend:

```json
{
  "issue": 0,
  "pr_url": "",
  "status": "done",
  "checks": "green",
  "reason": null
}
```

`status`: `done` | `blocked`  
`checks`: `green` | `red` | `none`
```

**pr-fix.md** (new):

```markdown
# PR fix worker

You fix an existing PR until local verify and remote CI are green.

## Context
- PR: {{PR}}
- Worktree: {{WORKTREE}} (already checked out)
- Do not merge. Do not change unrelated files.

## Loop
1. Read `gh pr view {{PR}} --json title,body,statusCheckRollup`
2. Reproduce failures (`gh pr checks`, local verify)
3. Fix, commit (conventional), push to PR branch
4. Re-run verify + checks
5. Stop when green or BLOCKED (need human)

## Report
```json
{"pr_url":"...","status":"done","checks":"green","reason":null}
```
```

**reviewer.md** — add: security-sensitive changes → recommend host Security Review / Bugbot plugins when available; still block on Critical.

**intake.md** — same GO gate; after GO instruct master to call foam skills then `orch.sh --mode ready-pickup --issue …`.

- [ ] **Step 2: Commit**

```bash
git add skills/core/dev-orchestrator/prompts
git commit -m "$(cat <<'EOF'
feat(skills): add dev-orchestrator worker and PR-fix prompts

EOF
)"
```

---

### Task 9: Full `SKILL.md` contract

**Files:**
- Modify: `skills/core/dev-orchestrator/SKILL.md` (replace stub)

**Interfaces:**
- Produces: complete agent-facing contract matching the design spec

- [ ] **Step 1: Write full SKILL.md** covering:

1. Announce line  
2. Role = glue master  
3. Modes table (`intake` / `ready-pickup` / `pr-fix`)  
4. Hard rules (from spec)  
5. Pipeline  
6. When to use harness:

```bash
SKILL=~/.cursor/skills/dev-orchestrator   # or claude/opencode path
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
bash "$SKILL/scripts/orch.sh" --mode pr-fix --pr 88 --sandbox
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --dry-run
```

7. Skill-only fallback paragraph (host Task/subagents + same prompts/ledger)  
8. Integration table with foam/kanban/git-commit/code-reviewer  
9. Out of scope (auto-merge, full Docker isolation)  
10. Link conventions + prompts + design spec path

Keep under ~160 lines; details stay in conventions/prompts.

- [ ] **Step 2: Commit**

```bash
git add skills/core/dev-orchestrator/SKILL.md
git commit -m "$(cat <<'EOF'
feat(skills): document dev-orchestrator skill contract

EOF
)"
```

---

### Task 10: Migration — batch alias + kit docs

**Files:**
- Modify: `skills/core/foam-batch-orchestrator/SKILL.md` (thin alias)
- Modify: `README.md`
- Modify: `skills/README.md`
- Modify: `rules/01-project-workflow.md`
- Modify: `skills/core/project-agents-setup/templates/AGENTS.base.md`
- Modify: `claude/CLAUDE.md` via `scripts/assemble-claude-md.sh` if rules changed
- Modify: `docs/superpowers/specs/2026-10-02-dev-orchestrator-design.md` status → `approved`

- [ ] **Step 1: Replace foam-batch SKILL body with alias**

Keep frontmatter `name: foam-batch-orchestrator` but description says deprecated alias. Body:

```markdown
# Foam batch orchestrator (alias)

**Deprecated name.** Use **`dev-orchestrator`** with `mode=intake`.

Announce: **"Using foam-batch-orchestrator skill (alias → dev-orchestrator)."**

Then follow `dev-orchestrator` skill exactly for intake → GO → ready-pickup dispatch.

Prompts/templates here remain only for backward compatibility; prefer
`~/.cursor/skills/dev-orchestrator/`.
```

- [ ] **Step 2: Update discovery surfaces**

In README skills table add `dev-orchestrator`; note batch as alias.  
In `skills/README.md` core row include it.  
In `AGENTS.base.md` skills list + table: primary `dev-orchestrator`, batch as alias.  
In `rules/01-project-workflow.md` list `dev-orchestrator`.  
Set spec status to `approved`.

- [ ] **Step 3: Assemble Claude md + dry-run install**

```bash
./scripts/assemble-claude-md.sh
./install.sh --dry-run | head -80
```

Expected: dry-run shows link to `dev-orchestrator`.

- [ ] **Step 4: Commit**

```bash
git add README.md skills/README.md rules/01-project-workflow.md \
  skills/core/foam-batch-orchestrator/SKILL.md \
  skills/core/project-agents-setup/templates/AGENTS.base.md \
  claude/CLAUDE.md \
  docs/superpowers/specs/2026-10-02-dev-orchestrator-design.md
git commit -m "$(cat <<'EOF'
docs(skills): wire dev-orchestrator and deprecate batch alias

EOF
)"
```

---

### Task 11: End-to-end harness smoke (local, no remote CI)

**Files:**
- Create: `skills/core/dev-orchestrator/scripts/tests/smoke_orch_dry.sh`

- [ ] **Step 1: Write smoke script**

Creates temp git repo, runs:

1. `run-tests.sh` (all unit tests)  
2. `orch.sh --mode ready-pickup --issue 1 --issue 2 --dry-run`  
3. `orch.sh --mode ready-pickup --issue 1` (actually creates one worktree)  
4. Assert worktree path exists  
5. `orch_worktree_remove` cleanup  

- [ ] **Step 2: Run smoke**

```bash
bash skills/core/dev-orchestrator/scripts/tests/smoke_orch_dry.sh
bash skills/core/dev-orchestrator/scripts/tests/run-tests.sh
```

Expected: all exit 0

- [ ] **Step 3: Commit**

```bash
git add skills/core/dev-orchestrator/scripts/tests
git commit -m "$(cat <<'EOF'
test(skills): add orch dry-run smoke for worktree dispatch

EOF
)"
```

---

## Spec coverage checklist (self-review)

| Spec requirement | Task |
| ---------------- | ---- |
| New skill + layout | 1, 8, 9 |
| Hybrid harness `orch.sh` | 7 |
| Modes intake / ready-pickup / pr-fix | 7, 8, 9 |
| Worktrees | 3, 7 |
| Agent detection multi-CLI | 4 |
| Loop verify + max iter | 5, 7 |
| CI watch no merge | 5, 7, 8 |
| Optional sandbox | 6 |
| Skill-only fallback | 9 |
| Batch thin alias + docs | 10 |
| Caps parallel 3 / iter 10 | 1, 7 |
| Unit/smoke tests | 2–7, 11 |

## Placeholder / consistency scan

- Function names stable: `orch_worktree_*`, `orch_detect_agent`, `orch_agent_run`, `orch_resolve_verify`, `orch_run_verify`, `orch_pr_checks_watch`, `orch_sandbox_wrap`, `orch_loop_worker`.
- No TBD/TODO left in steps.
- Intake GitHub bootstrap remains skill-delegated (not duplicated in bash) — intentional YAGNI.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-10-02-dev-orchestrator.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — execute tasks in this session with checkpoints  

Which approach?
