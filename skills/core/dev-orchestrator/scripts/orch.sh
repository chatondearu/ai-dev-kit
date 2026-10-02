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

export -f orch_loop_worker orch_agent_run orch_run_verify orch_resolve_verify _orch_extract_agents_verify orch_detect_agent orch_render_prompt orch_pr_mergeable orch_pr_checks_watch _orch_pr_checks_classify

MODE=""; REPO="$(pwd)"; DRY=0; SANDBOX=0; FAIL_FAST=0
MAX_P="${ORCH_MAX_PARALLEL:-3}"; MAX_I="${ORCH_MAX_ITER:-10}"
ISSUES=(); PR=""; FORCE_CLEAN=0

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
      if [ "$idx" -ge "$MAX_P" ]; then
        echo "cap ORCH_MAX_PARALLEL=$MAX_P reached" >&2
        break
      fi
      slug="issue-$id"
      branch="feat/${id}-${slug}"
      path="$(orch_worktree_path "$REPO" "$id" "$slug")"
      port=$(( ${ORCH_BASE_PORT:-3900} + idx ))
      echo "worker id=$id branch=$branch wt=$path port=$port"
      if [ "$DRY" = 0 ]; then
        if ! orch_worktree_add "$REPO" "$id" "$slug" "$branch" HEAD; then
          printf 'blocked: failed to create worktree for issue %s\n' "$id" >&2
          if [ "$FAIL_FAST" = 1 ]; then
            exit 1
          fi
          idx=$((idx + 1))
          continue
        fi
        orch_worktree_bootstrap "$REPO" "$path"

        mkdir -p "$path/.orch"
        worker_prompt="$path/.orch/worker-prompt.md"

        title="N/A"
        if command -v gh >/dev/null 2>&1; then
          title="$(gh issue view "$id" --json title -q .title 2>/dev/null || echo "N/A")"
        fi
        [ -z "$title" ] && title="N/A"

        plan_path="N/A"
        for p in "$REPO"/plans/PLAN-"$id"-*.md "$path"/plans/PLAN-"$id"-*.md; do
          if [ -f "$p" ]; then
            plan_path="${p#$REPO/}"
            break
          fi
        done

        orch_render_prompt "$ROOT/../prompts/worker.md" "$worker_prompt" \
          "ISSUE=$id" \
          "WORKTREE=$path" \
          "REPO_ROOT=$REPO" \
          "TITLE=$title" \
          "PLAN_PATH=$plan_path"

        export ORCH_WT="$path"
        orch_detect_agent >/dev/null
        loop_rc=0
        orch_sandbox_wrap bash -c "ORCH_WT=\"$path\" orch_loop_worker \"$path\" \"$worker_prompt\"" || loop_rc=$?
        if [ "$loop_rc" -ne 0 ]; then
          printf 'blocked: worker loop failed for issue %s\n' "$id" >&2
          if [ "$FAIL_FAST" = 1 ]; then
            exit 1
          fi
          idx=$((idx + 1))
          continue
        fi

        # Find PR number for branch
        pr_num=""
        if command -v gh >/dev/null 2>&1; then
          pr_json="$(gh pr list --head "$branch" --json number 2>/dev/null || true)"
          if command -v jq >/dev/null 2>&1; then
            pr_num="$(jq -r '.[0].number // empty' <<<"$pr_json" 2>/dev/null || true)"
          else
            pr_num="$(echo "$pr_json" | grep -oE '"number":[0-9]+' | head -n 1 | cut -d: -f2)"
          fi
          [ "$pr_num" = "null" ] && pr_num=""
        fi

        if [ -n "$pr_num" ]; then
          if ! orch_pr_checks_watch "$pr_num" 600; then
            printf 'blocked: CI checks failed or pending timeout for PR %s (issue %s)\n' "$pr_num" "$id" >&2
            if [ "$FAIL_FAST" = 1 ]; then
              exit 1
            fi
            idx=$((idx + 1))
            continue
          fi
          m="$(orch_pr_mergeable "$pr_num")" || true
          if [ "$m" != "MERGEABLE" ] && [ "$m" != "UNKNOWN" ]; then
            printf 'blocked: PR %s is not mergeable (mergeable=%s)\n' "$pr_num" "$m" >&2
            if [ "$FAIL_FAST" = 1 ]; then
              exit 1
            fi
            idx=$((idx + 1))
            continue
          fi
          printf 'done: issue %s (PR %s checks green, mergeable=%s)\n' "$id" "$pr_num" "$m"
        else
          printf 'done: issue %s (no PR found for branch %s)\n' "$id" "$branch"
        fi

        if [ "$FORCE_CLEAN" = 1 ]; then
          orch_worktree_remove "$REPO" "$path"
        fi
      fi
      idx=$((idx + 1))
    done
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

      orch_pr_checks_watch "$num" 600 || exit 1
      m="$(orch_pr_mergeable "$num")" || true
      if [ "$m" != "MERGEABLE" ] && [ "$m" != "UNKNOWN" ]; then
        printf 'blocked: PR %s is not mergeable (mergeable=%s)\n' "$num" "$m" >&2
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
