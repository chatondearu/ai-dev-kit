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
#   -h|--help
set -euo pipefail
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
source "$ROOT/lib/progress.sh"

MODE=""; REPO="$(pwd)"; DRY=0; SANDBOX=0; FAIL_FAST=0
MAX_P="${ORCH_MAX_PARALLEL:-3}"; MAX_I="${ORCH_MAX_ITER:-10}"
ISSUES=(); PR=""; FORCE_CLEAN=0; NO_PROGRESS=0

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
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; exit 2;;
  esac
  shift
done

export ORCH_MAX_PARALLEL="$MAX_P" ORCH_MAX_ITER="$MAX_I" ORCH_SANDBOX="$SANDBOX"
export ORCH_FAIL_FAST="$FAIL_FAST" ORCH_FORCE_CLEAN="$FORCE_CLEAN" ORCH_ROOT="$ROOT"
REPO="$(cd "$REPO" && pwd)"
export ORCH_REPO="$REPO"
if [ "$NO_PROGRESS" = 1 ]; then
  export ORCH_PROGRESS=0
fi

# Wait until fewer than MAX_P background jobs are running (bash job pool).
orch_wait_for_slot() {
  local max="$1"
  while [ "$(jobs -rp 2>/dev/null | wc -l | tr -d ' ')" -ge "$max" ]; do
    if ! wait -n 2>/dev/null; then
      # bash without wait -n, or no jobs left: wait for any one pid
      local pid
      pid="$(jobs -rp 2>/dev/null | head -n 1 || true)"
      if [ -n "$pid" ]; then
        wait "$pid" || true
      else
        break
      fi
    fi
  done
}

# Run one ready-pickup worker end-to-end. Writes $ORCH_REPO/.orch/run/<id>.rc (0|1).
orch_ready_pickup_one() {
  local id="$1" idx="$2"
  local slug="issue-$id"
  local branch="feat/${id}-${slug}"
  local path port worker_prompt title plan_path p
  local loop_rc=0 pr_num="" pr_json="" m=""
  local rc_dir rc_file

  export ORCH_WORKER_ID="$id"
  path="$(orch_worktree_path "$ORCH_REPO" "$id" "$slug")"
  port=$(( ${ORCH_BASE_PORT:-3900} + idx ))
  rc_dir="$ORCH_REPO/.orch/run"
  mkdir -p "$rc_dir"
  rc_file="$rc_dir/${id}.rc"

  orch_progress_set "$id" pending "branch ${branch}"
  if ! orch_progress_enabled; then
    echo "worker id=$id branch=$branch wt=$path port=$port"
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
  if command -v gh >/dev/null 2>&1; then
    title="$(gh issue view "$id" --json title -q .title 2>/dev/null || echo "N/A")"
  fi
  [ -z "$title" ] && title="N/A"

  plan_path="N/A"
  for p in "$ORCH_REPO"/plans/PLAN-"$id"-*.md "$path"/plans/PLAN-"$id"-*.md; do
    if [ -f "$p" ]; then
      plan_path="${p#$ORCH_REPO/}"
      break
    fi
  done

  orch_render_prompt "$ORCH_ROOT/../prompts/worker.md" "$worker_prompt" \
    "ISSUE=$id" \
    "WORKTREE=$path" \
    "REPO_ROOT=$ORCH_REPO" \
    "TITLE=$title" \
    "PLAN_PATH=$plan_path"

  export ORCH_WT="$path"
  orch_detect_agent >/dev/null
  orch_progress_set "$id" agent "starting"
  orch_sandbox_wrap bash -c "ORCH_WT=\"$path\" ORCH_WORKER_ID=\"$id\" ORCH_REPO=\"$ORCH_REPO\" ORCH_PROGRESS=\"${ORCH_PROGRESS:-auto}\" orch_loop_worker \"$path\" \"$worker_prompt\"" || loop_rc=$?
  if [ "$loop_rc" -ne 0 ]; then
    orch_progress_set "$id" blocked "worker loop failed"
    if ! orch_progress_enabled; then
      printf 'blocked: worker loop failed for issue %s\n' "$id" >&2
    fi
    echo 1 >"$rc_file"
    return 1
  fi

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

  orch_progress_set "$id" ci "PR #${pr_num} checks"
  if ! orch_pr_checks_watch "$pr_num" 600; then
    orch_progress_set "$id" blocked "CI failed/timeout PR #${pr_num}"
    if ! orch_progress_enabled; then
      printf 'blocked: CI checks failed or pending timeout for PR %s (issue %s)\n' "$pr_num" "$id" >&2
    fi
    echo 1 >"$rc_file"
    return 1
  fi
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

export -f orch_ready_pickup_one orch_wait_for_slot
export -f orch_loop_worker orch_agent_run orch_run_verify orch_resolve_verify
export -f _orch_extract_agents_verify orch_detect_agent orch_render_prompt
export -f orch_pr_mergeable orch_pr_checks_watch _orch_pr_checks_classify
export -f orch_worktree_path orch_worktree_add orch_worktree_bootstrap orch_worktree_remove
export -f orch_sandbox_wrap
export -f orch_progress_dir orch_progress_enabled orch_progress_set
export -f orch_progress_icon orch_progress_draw

case "$MODE" in
  ready-pickup)
    [ "${#ISSUES[@]}" -gt 0 ] || { echo "need --issue"; exit 2; }
    idx=0
    any_blocked=0

    if [ "$DRY" = 1 ]; then
      for id in "${ISSUES[@]}"; do
        slug="issue-$id"
        branch="feat/${id}-${slug}"
        path="$(orch_worktree_path "$REPO" "$id" "$slug")"
        port=$(( ${ORCH_BASE_PORT:-3900} + idx ))
        echo "worker id=$id branch=$branch wt=$path port=$port"
        idx=$((idx + 1))
      done
      exit 0
    fi

    mkdir -p "$REPO/.orch/run"
    rm -f "$REPO/.orch/run"/*.rc "$REPO/.orch/run"/*.status "$REPO/.orch/run"/.dashboard-stop 2>/dev/null || true

    export ORCH_PROGRESS_IDS="${ISSUES[*]}"
    for id in "${ISSUES[@]}"; do
      orch_progress_set "$id" pending "queued"
    done
    orch_progress_dashboard_start

    if [ "$MAX_P" -le 1 ]; then
      for id in "${ISSUES[@]}"; do
        if ! orch_ready_pickup_one "$id" "$idx"; then
          any_blocked=1
          if [ "$FAIL_FAST" = 1 ]; then
            orch_progress_dashboard_stop
            exit 1
          fi
        fi
        idx=$((idx + 1))
      done
    else
      if ! orch_progress_enabled; then
        printf 'orch: dispatching %s workers with ORCH_MAX_PARALLEL=%s\n' "${#ISSUES[@]}" "$MAX_P" >&2
      fi
      # Monitor mode required for jobs/wait -n in non-interactive scripts
      set -m
      pids=()
      for id in "${ISSUES[@]}"; do
        while [ "${#pids[@]}" -ge "$MAX_P" ]; do
          if wait -n 2>/dev/null; then
            :
          else
            still=()
            for pid in "${pids[@]}"; do
              if kill -0 "$pid" 2>/dev/null; then
                still+=("$pid")
              else
                wait "$pid" || true
              fi
            done
            pids=("${still[@]}")
            if [ "${#pids[@]}" -ge "$MAX_P" ]; then
              sleep 0.05
            fi
          fi
          still=()
          for pid in "${pids[@]}"; do
            if kill -0 "$pid" 2>/dev/null; then
              still+=("$pid")
            fi
          done
          pids=("${still[@]}")
        done
        (
          set +e
          orch_ready_pickup_one "$id" "$idx"
          exit $?
        ) &
        pids+=($!)
        idx=$((idx + 1))
        if [ "$FAIL_FAST" = 1 ]; then
          for rc_file in "$REPO/.orch/run"/*.rc; do
            [ -f "$rc_file" ] || continue
            if [ "$(cat "$rc_file")" != "0" ]; then
              for pid in "${pids[@]}"; do wait "$pid" || true; done
              orch_progress_dashboard_stop
              exit 1
            fi
          done
        fi
      done
      for pid in "${pids[@]}"; do
        wait "$pid" || true
      done
      set +m
    fi

    orch_progress_dashboard_stop

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

    if [ "$any_blocked" -ne 0 ]; then
      exit 1
    fi
    ;;
  pr-fix)
    [ -n "$PR" ] || { echo "need --pr"; exit 2; }
    echo "pr-fix pr=$PR dry=$DRY"
    if [ "$DRY" = 0 ]; then
      pr_clean="${PR%/}"
      while [ "${pr_clean%/}" != "$pr_clean" ]; do
        pr_clean="${pr_clean%/}"
      done
      num="${pr_clean##*/}"
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

      head_ref=""
      if command -v gh >/dev/null 2>&1; then
        head_ref="$(gh pr view "$num" --json headRefName -q .headRefName 2>/dev/null || true)"
      fi
      [ -z "$head_ref" ] && head_ref="orch/pr-$num"

      mkdir -p "$path/.orch"
      prompt_file="$path/.orch/pr-fix-prompt.md"
      orch_render_prompt "$ROOT/../prompts/pr-fix.md" "$prompt_file" \
        "PR=$PR" \
        "WORKTREE=$path" \
        "HEAD_REF=$head_ref"

      export ORCH_WT="$path"
      orch_detect_agent >/dev/null
      loop_rc=0
      orch_sandbox_wrap bash -c "ORCH_WT=\"$path\" orch_loop_worker \"$path\" \"$prompt_file\"" || loop_rc=$?
      if [ "$loop_rc" -ne 0 ]; then
        printf 'blocked: worker loop failed for PR %s\n' "$num" >&2
        exit 1
      fi

      if ! orch_pr_checks_watch "$num" 600; then
        printf 'blocked: CI checks failed or pending timeout for PR %s\n' "$num" >&2
        exit 1
      fi
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
