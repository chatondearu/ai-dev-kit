#!/usr/bin/env bash
# orch.sh --mode ready-pickup|pr-fix|intake [options]
#   --repo DIR           (default: cwd)
#   --issue N            (repeatable for ready-pickup)
#   --pr N|URL           (pr-fix)
#   --agent CMD          (sets ORCH_AGENT)
#   --sandbox            (ORCH_SANDBOX=1)
#   --max-parallel N
#   --max-iter N
#   --fail-fast          (abort on first worker failure instead of continuing)
#   --dry-run            (print plan: worktrees/branches; no agent invoke)
#   --force-clean        (remove worktrees after — only with explicit flag)
#   --graph-plan FILE    (intra-issue fan-out from JSON plan; opt-in)
#   --no-graph           (disable live graph updates under .orch/graph/)
#   -h|--help
set -euo pipefail
# A role inherited from a calling agent (e.g. a pr-fix worker running orch) is not ours.
unset ORCH_ROLE
ROOT="$(cd "$(dirname "$0")" && pwd)"
PROMPTS_DIR="$(cd "$ROOT/../prompts" && pwd)"
export ORCH_PROMPTS_DIR="$PROMPTS_DIR"

# shellcheck source=lib/worktree.sh
source "$ROOT/lib/worktree.sh"
source "$ROOT/lib/agents.sh"
source "$ROOT/lib/verify.sh"
source "$ROOT/lib/ci.sh"
source "$ROOT/lib/sandbox.sh"
source "$ROOT/lib/loop.sh"
source "$ROOT/lib/graph.sh"
source "$ROOT/lib/progress.sh"
source "$ROOT/lib/frontier.sh"
source "$ROOT/lib/lock.sh"

MODE=""; REPO="$(pwd)"; DRY=0; SANDBOX=0; FAIL_FAST=0
MAX_P="${ORCH_MAX_PARALLEL:-3}"; MAX_I="${ORCH_MAX_ITER:-10}"
ISSUES=(); PR=""; FORCE_CLEAN=0; NO_PROGRESS=0; GRAPH_PLAN=""

usage() {
  cat <<'EOF'
orch.sh --mode ready-pickup|pr-fix|intake [options]
  --repo DIR           (default: cwd)
  --issue N            (repeatable for ready-pickup)
  --pr N|URL           (pr-fix)
  --agent CMD          (sets ORCH_AGENT)
  --sandbox            (ORCH_SANDBOX=1)
  --max-parallel N
  --max-iter N
  --fail-fast          (abort on first worker failure instead of continuing)
  --dry-run            (print plan: worktrees/branches; no agent invoke)
  --force-clean        (remove worktrees after — only with explicit flag)
  --no-progress        (disable TTY progress dashboard; ORCH_PROGRESS=0)
  --no-graph           (disable live graph updates under .orch/graph/)
  --graph-plan FILE    (intra-issue task fan-out JSON; opt-in)
  -h|--help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --mode) MODE="$2"; shift;;
    --repo) REPO="$2"; shift;;
    --issue) ISSUES+=("$2"); shift;;
    --pr) PR="$2"; shift;;
    --agent) ORCH_AGENT="$2"; export ORCH_AGENT; shift;;
    --sandbox) SANDBOX=1;;
    --max-parallel) MAX_P="$2"; shift;;
    --max-iter) MAX_I="$2"; shift;;
    --fail-fast) FAIL_FAST=1;;
    --dry-run) DRY=1;;
    --force-clean) FORCE_CLEAN=1;;
    --no-progress) NO_PROGRESS=1;;
    --no-graph) export ORCH_GRAPH=0;;
    --graph-plan) GRAPH_PLAN="$2"; shift;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; exit 2;;
  esac
  shift
done

# Ctrl-C / SIGTERM must not orphan agents running with --force / --dangerously-skip-permissions.
# Kills this process group (workers, agents, dashboard). Armed after the libs are sourced, so
# sourcing them from a test never installs it. Run orch in its own group (`setsid`, or `set -m`)
# when launching it from a script that must survive an interrupt.
# ORCH_TMUX=1 agents live in tmux, outside this group: kill their sessions too.
trap '_orch_release_lock; orch_tmux_kill_run; trap - INT TERM; kill 0' INT TERM

export ORCH_MAX_PARALLEL="$MAX_P" ORCH_MAX_ITER="$MAX_I" ORCH_SANDBOX="$SANDBOX"
export ORCH_FAIL_FAST="$FAIL_FAST" ORCH_FORCE_CLEAN="$FORCE_CLEAN" ORCH_ROOT="$ROOT"
REPO="$(orch_repo_root "$REPO")"
export ORCH_REPO="$REPO"
if [ "$NO_PROGRESS" = 1 ]; then
  export ORCH_PROGRESS=0
fi
# One run id for the graph file (.orch/graph/live/<run>.json), status.json and tmux
# worker sessions (orch-<run>-<worker>), shared by every worker.
export ORCH_RUN_ID="${ORCH_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
if [ "${ORCH_TMUX:-0}" = 1 ]; then
  [ "$SANDBOX" != 1 ] || echo "orch: ORCH_TMUX ignored with --sandbox (tmux runs agents outside bwrap)" >&2
fi

# Project opt-in: AGENTS.md line `orch: allow-no-checks` (repos without Actions).
orch_apply_agents_orch_flags() {
  local agents="${ORCH_REPO}/AGENTS.md"
  [ -f "$agents" ] || return 0
  if grep -Eiq '^[[:space:]]*orch:[[:space:]]*allow-no-checks\b' "$agents"; then
    export ORCH_ALLOW_NO_CHECKS=1
  fi
}
orch_apply_agents_orch_flags

# GitHub author associations allowed to write the worker contract (brief, body markers).
_orch_trusted_assoc() {
  case "${1:-}" in OWNER|MEMBER|COLLABORATOR) return 0 ;; esac
  return 1
}

# True if issue $1 was opened by a trusted author (fail closed on gh errors).
orch_issue_author_trusted() {
  local assoc
  assoc="$(gh api "repos/{owner}/{repo}/issues/$1" -q .author_association 2>/dev/null || true)"
  _orch_trusted_assoc "$assoc"
}

# Write the latest triage agent brief for issue $1 into $2. Returns 0 if found.
# Only OWNER/MEMBER/COLLABORATOR comments count: anyone else could write the contract.
orch_fetch_agent_brief() {
  local id="$1" dest="$2" body="" raw=""
  mkdir -p "$(dirname "$dest")"
  if ! command -v gh >/dev/null 2>&1 || ! command -v jq >/dev/null 2>&1; then
    return 1
  fi
  # Do not pass --comments with --json (gh rejects the combination).
  raw="$(gh issue view "$id" --json comments 2>/dev/null || true)"
  [ -n "$raw" ] || return 1
  body="$(printf '%s' "$raw" | jq -r \
    '[.comments[] | select(.authorAssociation | IN("OWNER","MEMBER","COLLABORATOR")) | .body]
     | map(select(test("## Agent Brief"))) | last // empty' \
    2>/dev/null || true)"
  if [ -z "$body" ] || [ "$body" = "null" ]; then
    return 1
  fi
  printf '%s\n' "$body" >"$dest"
  return 0
}

# Run one ready-pickup worker end-to-end. Writes $ORCH_REPO/.orch/run/<id>.rc (0|1).
orch_ready_pickup_one() {
  local id="$1"
  local slug="issue-$id"
  local branch="feat/${id}-${slug}"
  local path worker_prompt title plan_path p
  local loop_rc=0 pr_num="" pr_json="" m=""
  local rc_dir rc_file
  local lane="full" brief_path="N/A" brief_file=""
  local allow_no_verify=0 review_prompt review_log fix_prompt

  export ORCH_WORKER_ID="$id"
  path="$(orch_worktree_path "$ORCH_REPO" "$id" "$slug")"
  rc_dir="$ORCH_REPO/.orch/run"
  mkdir -p "$rc_dir"
  rc_file="$rc_dir/${id}.rc"

  orch_progress_set "$id" pending "branch ${branch}"
  if ! orch_progress_enabled; then
    echo "worker id=$id branch=$branch wt=$path"
  fi

  orch_progress_set "$id" worktree "creating ${path##*/}"
  if ! orch_worktree_add "$ORCH_REPO" "$id" "$slug" "$branch" HEAD; then
    orch_progress_set "$id" blocked "worktree create failed"
    if ! orch_progress_enabled; then
      printf 'blocked: failed to create worktree for issue %s\n' "$id" >&2
    fi
    echo 1 >"$rc_file"
    return 1
  fi
  orch_worktree_bootstrap "$ORCH_REPO" "$path"

  mkdir -p "$path/.orch"
  worker_prompt="$path/.orch/worker-prompt.md"

  title="N/A"
  local body=""
  if command -v gh >/dev/null 2>&1; then
    title="$(gh issue view "$id" --json title -q .title 2>/dev/null || echo "N/A")"
    body="$(gh issue view "$id" --json body -q .body 2>/dev/null || true)"
    # Escape hatch (ADR-006): `orch: allow-no-verify` in the body keeps verify=true
    # when no verify command exists; otherwise a missing command blocks.
    # Honoured only when the issue author is trusted (OWNER/MEMBER/COLLABORATOR).
    if printf '%s' "$body" | grep -Fq 'orch: allow-no-verify'; then
      if orch_issue_author_trusted "$id"; then
        allow_no_verify=1
      else
        printf 'orch: ignoring `orch: allow-no-verify` on issue %s (author not OWNER/MEMBER/COLLABORATOR)\n' "$id" >&2
      fi
    fi
  fi
  orch_resolve_context "$ORCH_REPO" "$id" "$body"
  [ -z "$title" ] && title="N/A"

  plan_path="N/A"
  for p in \
    "$ORCH_REPO"/plans/PLAN-"$id"-*.md \
    "$path"/plans/PLAN-"$id"-*.md
  do
    if [ -f "$p" ]; then
      case "$p" in
        "$path"/*) plan_path="${p#$path/}" ;;
        *) plan_path="${p#$ORCH_REPO/}" ;;
      esac
      break
    fi
  done

  # Fast-lane: no PLAN → triage agent brief is the contract (ADR-002).
  if [ "$plan_path" = "N/A" ]; then
    lane="fast"
    brief_file="$path/.orch/brief.md"
    if ! orch_fetch_agent_brief "$id" "$brief_file"; then
      orch_progress_set "$id" blocked "fast-lane needs agent brief"
      if ! orch_progress_enabled; then
        printf 'blocked: issue %s has no PLAN and no agent brief\n' "$id" >&2
      fi
      echo 1 >"$rc_file"
      return 1
    fi
    brief_path="${brief_file#$ORCH_REPO/}"
    # Prefer path relative to worktree when under it
    case "$brief_file" in
      "$path"/*) brief_path="${brief_file#$path/}" ;;
    esac
  fi

  orch_render_prompt "$ORCH_ROOT/../prompts/worker.md" "$worker_prompt" \
    "ISSUE=$id" \
    "BRANCH=$branch" \
    "WORKTREE=$path" \
    "REPO_ROOT=$ORCH_REPO" \
    "TITLE=$title" \
    "PLAN_PATH=$plan_path" \
    "BRIEF_PATH=$brief_path" \
    "LANE=$lane" \
    "PRD_PATH=$ORCH_PRD_PATH" \
    "EPIC=$ORCH_EPIC" \
    "CONSTRAINTS=$ORCH_CONSTRAINTS"

  export ORCH_WT="$path"
  orch_resolve_agent worker >/dev/null
  orch_progress_set "$id" agent "starting"
  if [ -f "$path/.orch/graph-plan-complete" ]; then
    {
      cat "$worker_prompt"
      printf '\n\n## Graph plan\nImplementation tasks already ran via --graph-plan. Complete review, PR, and CI only.\n'
    } >"$path/.orch/worker-prompt-handoff.md"
    worker_prompt="$path/.orch/worker-prompt-handoff.md"
  fi
  local worker_env="ORCH_WT=\"$path\" ORCH_WORKER_ID=\"$id\" ORCH_REPO=\"$ORCH_REPO\" ORCH_PROGRESS=\"${ORCH_PROGRESS:-auto}\" ORCH_ALLOW_NO_VERIFY=\"$allow_no_verify\""
  orch_sandbox_wrap bash -c "$worker_env orch_loop_worker \"$path\" \"$worker_prompt\"" || loop_rc=$?
  if [ "$loop_rc" -ne 0 ]; then
    # rc 3 = no verify command (loop already set the progress reason).
    if [ "$loop_rc" -ne 3 ]; then
      orch_progress_set "$id" blocked "worker loop failed"
    fi
    if ! orch_progress_enabled; then
      if [ "$loop_rc" -eq 3 ]; then
        printf 'blocked: issue %s %s\n' "$id" "$ORCH_NO_VERIFY_REASON" >&2
      else
        printf 'blocked: worker loop failed for issue %s\n' "$id" >&2
      fi
    fi
    echo 1 >"$rc_file"
    return 1
  fi
  orch_worker_result_warn "$path"

  orch_progress_set "$id" ci "looking up PR for ${branch}"
  if command -v gh >/dev/null 2>&1; then
    pr_json="$(gh pr list --head "$branch" --json number 2>/dev/null || true)"
    if command -v jq >/dev/null 2>&1; then
      pr_num="$(jq -r '.[0].number // empty' <<<"$pr_json" 2>/dev/null || true)"
    else
      pr_num="$(echo "$pr_json" | grep -oE '"number":[0-9]+' | head -n 1 | cut -d: -f2)"
    fi
    [ "$pr_num" = "null" ] && pr_num=""
  fi

  if [ -z "$pr_num" ]; then
    orch_progress_set "$id" blocked "no PR for ${branch}"
    if ! orch_progress_enabled; then
      printf 'blocked: issue %s (no PR for branch %s)\n' "$id" "$branch" >&2
    fi
    echo 1 >"$rc_file"
    return 1
  fi

  # Independent review gate (ADR-006): reviewer.md in a fresh agent context.
  # changes_requested → exactly one fix iteration, one re-review, then blocked.
  local round
  for round in 1 2; do
    review_prompt="$path/.orch/reviewer-prompt-${round}.md"
    review_log="$path/.orch/review-${round}.log"
    orch_render_prompt "$ORCH_ROOT/../prompts/reviewer.md" "$review_prompt" \
      "ISSUE=$id" \
      "WORKTREE=$path" \
      "REPO_ROOT=$ORCH_REPO" \
      "TITLE=$title" \
      "PLAN_PATH=$plan_path" \
      "BRIEF_PATH=$brief_path" \
      "PRD_PATH=$ORCH_PRD_PATH"
    orch_progress_set "$id" review "reviewer round ${round} (PR #${pr_num})"
    if orch_sandbox_wrap bash -c "$worker_env orch_review_run \"$path\" \"$review_prompt\" \"$review_log\""; then
      break
    fi
    if [ "$round" -eq 2 ]; then
      orch_progress_set "$id" blocked "review: changes_requested"
      if ! orch_progress_enabled; then
        printf 'blocked: issue %s review: changes_requested (see %s)\n' "$id" "$review_log" >&2
      fi
      echo 1 >"$rc_file"
      return 1
    fi
    fix_prompt="$(orch_review_fix_prompt "$worker_prompt" "$path" "$review_log")"
    orch_progress_set "$id" agent "review fix iteration"
    loop_rc=0
    orch_sandbox_wrap bash -c "$worker_env ORCH_MAX_ITER=1 orch_loop_worker \"$path\" \"$fix_prompt\"" || loop_rc=$?
    if [ "$loop_rc" -ne 0 ]; then
      orch_progress_set "$id" blocked "review fix iteration failed"
      if ! orch_progress_enabled; then
        printf 'blocked: review fix iteration failed for issue %s\n' "$id" >&2
      fi
      echo 1 >"$rc_file"
      return 1
    fi
  done

  orch_progress_set "$id" ci "PR #${pr_num} checks"
  local ci_status=""
  ci_status="$(orch_pr_checks_watch "$pr_num" 600)" || {
    if [ "$ci_status" = "none" ]; then
      orch_progress_set "$id" blocked "no checks on PR #${pr_num} (set orch: allow-no-checks)"
      if ! orch_progress_enabled; then
        printf 'blocked: PR %s has no checks (issue %s); set ORCH_ALLOW_NO_CHECKS=1 or AGENTS.md `orch: allow-no-checks`\n' \
          "$pr_num" "$id" >&2
      fi
    else
      orch_progress_set "$id" blocked "CI failed/timeout PR #${pr_num}"
      if ! orch_progress_enabled; then
        printf 'blocked: CI checks failed or pending timeout for PR %s (issue %s)\n' "$pr_num" "$id" >&2
      fi
    fi
    echo 1 >"$rc_file"
    return 1
  }
  if ! m="$(orch_pr_mergeable "$pr_num")" || { [ "$m" != "MERGEABLE" ] && [ "$m" != "UNKNOWN" ]; }; then
    orch_progress_set "$id" blocked "not mergeable (${m:-UNKNOWN})"
    if ! orch_progress_enabled; then
      printf 'blocked: PR %s is not mergeable (mergeable=%s)\n' "$pr_num" "${m:-UNKNOWN}" >&2
    fi
    echo 1 >"$rc_file"
    return 1
  fi
  orch_progress_set "$id" done "PR #${pr_num} green (${m})"
  if ! orch_progress_enabled; then
    printf 'done: issue %s (PR %s checks green, mergeable=%s)\n' "$id" "$pr_num" "$m"
  fi

  if [ "${ORCH_FORCE_CLEAN:-0}" = "1" ]; then
    orch_worktree_remove "$ORCH_REPO" "$path"
  fi
  echo 0 >"$rc_file"
  return 0
}

orch_export_functions
[ "$DRY" != 1 ] || orch_print_agents

# Remove worktrees of issues that ended `done`; blocked ones stay for inspection.
orch_cleanup_done() {
  local id names=()
  for id in "$@"; do
    if [ "$(cat "$REPO/.orch/run/${id}.rc" 2>/dev/null)" = "0" ]; then
      names+=("$(basename "$(orch_worktree_path "$REPO" "$id" "issue-$id")")")
    fi
  done
  [ "${#names[@]}" -eq 0 ] || bash "$ROOT/orch-cleanup.sh" --repo "$REPO" --apply "${names[@]}" || true
}

case "$MODE" in
  ready-pickup)
    [ "${#ISSUES[@]}" -gt 0 ] || { echo "need --issue"; exit 2; }
    if [ "$DRY" != 1 ]; then
      orch_acquire_lock
    fi
    any_blocked=0
    HELD_IDS=(); HELD_REASON=()

    # Frontier gating: hold any requested child whose `Blocked by:` issues are
    # not all closed; only the frontier is dispatched. Held issues are surfaced
    # on the progress board and in the summary below (not here — the progress dir
    # is reset further down). Skipped in --dry-run, when ORCH_FRONTIER=0, or when
    # gh is unavailable.
    if [ "$DRY" != 1 ] && [ "${ORCH_FRONTIER:-auto}" != 0 ] && command -v gh >/dev/null 2>&1; then
      GH_SLUG="$(orch_gh_repo_slug "$REPO" || true)"
      if [ -z "$GH_SLUG" ]; then
        printf 'orch: cannot resolve GitHub repo slug for %s (gh auth?)\n' "$REPO" >&2
        exit 1
      fi
      export ORCH_GH_REPO="$GH_SLUG"
      orch_frontier_partition "$GH_SLUG" "${ISSUES[@]}"
      if [ "${#HELD_IDS[@]}" -gt 0 ]; then
        for i in "${!HELD_IDS[@]}"; do
          printf 'orch: holding #%s (blocked by open #%s)\n' "${HELD_IDS[$i]}" "${HELD_REASON[$i]}" >&2
        done
        any_blocked=1
      fi
      if [ "${#FRONTIER_IDS[@]}" -eq 0 ]; then
        printf 'orch: no frontier — every requested issue is blocked\n' >&2
        exit 1
      fi
      ISSUES=("${FRONTIER_IDS[@]}")
    fi

    if [ -n "$GRAPH_PLAN" ]; then
      if [ "${#ISSUES[@]}" -ne 1 ]; then
        echo "graph-plan requires exactly one --issue" >&2
        exit 2
      fi
      if [ "$DRY" = 1 ]; then
        echo "graph-plan issue=${ISSUES[0]} file=$GRAPH_PLAN"
        jq . "$GRAPH_PLAN" 2>/dev/null || cat "$GRAPH_PLAN"
        exit 0
      fi
      if ! orch_graph_plan_run "$GRAPH_PLAN" "${ISSUES[0]}"; then
        orch_graph_finish blocked
        exit 1
      fi
      if ! orch_ready_pickup_one "${ISSUES[0]}"; then
        orch_graph_finish blocked
        exit 1
      fi
      orch_graph_finish ok
      orch_cleanup_done "${ISSUES[0]}"
      exit 0
    fi

    if [ "$DRY" = 1 ]; then
      for id in "${ISSUES[@]}"; do
        slug="issue-$id"
        branch="feat/${id}-${slug}"
        path="$(orch_worktree_path "$REPO" "$id" "$slug")"
        echo "worker id=$id branch=$branch wt=$path"
      done
      exit 0
    fi

    mkdir -p "$REPO/.orch/run"
    rm -f "$REPO/.orch/run"/*.rc "$REPO/.orch/run"/*.status "$REPO/.orch/run"/*.json "$REPO/.orch/run"/*.log "$REPO/.orch/run"/.dashboard-stop 2>/dev/null || true
    if [ "$DRY" != 1 ]; then
      orch_graph_init "$ORCH_RUN_ID"
      orch_graph_upsert_node integrator integrator pending queued "awaiting workers" ""
    fi

    if [ "${#HELD_IDS[@]}" -gt 0 ]; then
      export ORCH_PROGRESS_IDS="${ISSUES[*]} ${HELD_IDS[*]}"
    else
      export ORCH_PROGRESS_IDS="${ISSUES[*]}"
    fi
    for id in "${ISSUES[@]}"; do
      orch_progress_set "$id" pending "queued"
    done
    # Surface held children on the board (survives the .status reset above).
    for i in "${!HELD_IDS[@]}"; do
      orch_progress_set "${HELD_IDS[$i]}" blocked "waiting on #${HELD_REASON[$i]}"
    done
    orch_progress_dashboard_start

    if [ "$MAX_P" -le 1 ]; then
      for id in "${ISSUES[@]}"; do
        if ! orch_ready_pickup_one "$id"; then
          any_blocked=1
          if [ "$FAIL_FAST" = 1 ]; then
            orch_progress_dashboard_stop
            exit 1
          fi
        fi
      done
    else
      if ! orch_progress_enabled; then
        printf 'orch: dispatching %s workers with ORCH_MAX_PARALLEL=%s\n' "${#ISSUES[@]}" "$MAX_P" >&2
      fi
      for id in "${ISSUES[@]}"; do
        orch_pool_slot "$MAX_P"
        (
          set +e
          orch_ready_pickup_one "$id"
          exit $?
        ) &
        orch_pool_add $!
        if [ "$FAIL_FAST" = 1 ]; then
          for rc_file in "$REPO/.orch/run"/*.rc; do
            [ -f "$rc_file" ] || continue
            if [ "$(cat "$rc_file")" != "0" ]; then
              orch_pool_drain
              orch_progress_dashboard_stop
              exit 1
            fi
          done
        fi
      done
      orch_pool_drain
    fi

    orch_progress_dashboard_stop

    if [ "$DRY" != 1 ]; then
      orch_graph_integrator_run "${ISSUES[@]}" || true
    fi

    for rc_file in "$REPO/.orch/run"/*.rc; do
      [ -f "$rc_file" ] || continue
      if [ "$(cat "$rc_file")" != "0" ]; then
        any_blocked=1
      fi
    done
    for id in "${ISSUES[@]}"; do
      if [ ! -f "$REPO/.orch/run/${id}.rc" ]; then
        orch_progress_set "$id" blocked "no worker status file"
        any_blocked=1
      fi
    done

    # Plain summary after dashboard (or when progress off)
    for id in "${ISSUES[@]}"; do
      st="$REPO/.orch/run/${id}.status"
      rc="$REPO/.orch/run/${id}.rc"
      if [ -f "$st" ]; then
        line="$(cat "$st")"
        printf 'summary #%s %s — %s\n' "$id" "${line%%|*}" "${line#*|}"
      elif [ -f "$rc" ] && [ "$(cat "$rc")" = "0" ]; then
        printf 'summary #%s done\n' "$id"
      else
        printf 'summary #%s blocked\n' "$id"
      fi
    done
    # Held-by-dependency children (never dispatched this run).
    for i in "${!HELD_IDS[@]}"; do
      printf 'summary #%s blocked — waiting on #%s\n' "${HELD_IDS[$i]}" "${HELD_REASON[$i]}"
    done

    orch_cleanup_done "${ISSUES[@]}"

    if [ "$any_blocked" -ne 0 ]; then
      orch_graph_finish blocked
      exit 1
    fi
    orch_graph_finish ok
    ;;
  pr-fix)
    [ -n "$PR" ] || { echo "need --pr"; exit 2; }
    echo "pr-fix pr=$PR dry=$DRY"
    if [ "$DRY" = 0 ]; then
      orch_acquire_lock
      export ORCH_VERIFY_TRUST=untrusted
      pr_clean="${PR%/}"
      while [ "${pr_clean%/}" != "$pr_clean" ]; do
        pr_clean="${pr_clean%/}"
      done
      num="${pr_clean##*/}"
      # Runs before any checkout so untrusted PR code is never fetched for a blocked run.
      # The agent pushes `origin HEAD:<head_ref>`: a fork PR's origin is the base repo,
      # and a head equal to the default branch would push onto it. Block both.
      head_ref=""
      if command -v gh >/dev/null 2>&1; then
        head_ref="$(gh pr view "$num" --json headRefName -q .headRefName 2>/dev/null || true)"
        cross="$(gh pr view "$num" --json isCrossRepository -q .isCrossRepository 2>/dev/null || true)"
        default_branch="$(gh repo view --json defaultBranchRef -q .defaultBranchRef.name 2>/dev/null || true)"
        if [ -z "$head_ref" ] || [ -z "$cross" ] || [ -z "$default_branch" ]; then
          printf 'blocked: PR %s metadata unreadable (head ref, fork flag or default branch); not running the agent\n' "$num" >&2
          exit 1
        fi
        if [ "$cross" != false ]; then
          printf 'blocked: PR %s is cross-repository (fork); pr-fix only pushes same-repo branches\n' "$num" >&2
          exit 1
        fi
        if [ "$head_ref" = "$default_branch" ]; then
          printf 'blocked: PR %s head branch is the default branch (%s); refusing to push onto it\n' "$num" "$default_branch" >&2
          exit 1
        fi
      fi
      [ -z "$head_ref" ] && head_ref="orch/pr-$num"

      path="$(orch_worktree_path "$REPO" "pr-$num" "fix")"
      if [ ! -e "$path" ]; then
        mkdir -p "$(dirname "$path")"
        checked_out=0
        if command -v gh >/dev/null 2>&1; then
          if git -C "$REPO" worktree add --detach "$path" HEAD 2>/dev/null; then
            if (cd "$path" && gh pr checkout "$num" 2>/dev/null); then
              checked_out=1
            else
              git -C "$REPO" worktree remove --force "$path" 2>/dev/null || rm -rf "$path"
            fi
          fi
        fi
        if [ "$checked_out" = 0 ]; then
          git -C "$REPO" fetch -q origin "+pull/$num/head:orch/pr-$num" 2>/dev/null || git -C "$REPO" fetch -q origin "pull/$num/head:orch/pr-$num" || true
          git -C "$REPO" worktree add "$path" "orch/pr-$num"
        fi
      fi

      mkdir -p "$path/.orch"
      prompt_file="$path/.orch/pr-fix-prompt.md"
      orch_render_prompt "$ROOT/../prompts/pr-fix.md" "$prompt_file" \
        "PR=$PR" \
        "WORKTREE=$path" \
        "HEAD_REF=$head_ref"

      export ORCH_WT="$path"
      orch_resolve_agent pr-fix >/dev/null
      loop_rc=0
      orch_sandbox_wrap bash -c "ORCH_WT=\"$path\" ORCH_ROLE=pr-fix orch_loop_worker \"$path\" \"$prompt_file\"" || loop_rc=$?
      if [ "$loop_rc" -ne 0 ]; then
        printf 'blocked: worker loop failed for PR %s\n' "$num" >&2
        exit 1
      fi

      ci_status="$(orch_pr_checks_watch "$num" 600)" || {
        if [ "$ci_status" = "none" ]; then
          printf 'blocked: PR %s has no checks; set ORCH_ALLOW_NO_CHECKS=1 or AGENTS.md `orch: allow-no-checks`\n' "$num" >&2
        else
          printf 'blocked: CI checks failed or pending timeout for PR %s\n' "$num" >&2
        fi
        exit 1
      }
      m=""
      if ! m="$(orch_pr_mergeable "$num")" || { [ "$m" != "MERGEABLE" ] && [ "$m" != "UNKNOWN" ]; }; then
        printf 'blocked: PR %s is not mergeable (mergeable=%s)\n' "$num" "${m:-UNKNOWN}" >&2
        exit 1
      fi
      printf 'done: PR %s checks green, mergeable=%s\n' "$num" "$m"

      if [ "$FORCE_CLEAN" = 1 ]; then
        orch_worktree_remove "$REPO" "$path"
      fi
    fi
    ;;
  intake)
    echo "intake bootstrap is skill-driven; after GO use ready-pickup with child issue numbers" >&2
    exit 0
    ;;
  *) echo "need --mode"; exit 2;;
esac
