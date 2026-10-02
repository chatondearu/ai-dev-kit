#!/usr/bin/env bash
# orch.sh --mode ready-pickup|pr-fix|intake [options]
#   --repo DIR           (default: cwd)
#   --issue N            (repeatable for ready-pickup)
#   --pr N|URL           (pr-fix)
#   --agent CMD          (sets ORCH_AGENT)
#   --sandbox            (ORCH_SANDBOX=1)
#   --max-parallel N
#   --max-iter N
#   --dry-run            (print plan: worktrees/branches; no agent invoke)
#   --force-clean        (remove worktrees after — only with explicit flag)
#   -h|--help
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"

# shellcheck source=lib/worktree.sh
source "$ROOT/lib/worktree.sh"
source "$ROOT/lib/agents.sh"
source "$ROOT/lib/verify.sh"
source "$ROOT/lib/ci.sh"
source "$ROOT/lib/sandbox.sh"
source "$ROOT/lib/loop.sh"

export -f orch_loop_worker orch_agent_run orch_run_verify orch_resolve_verify orch_detect_agent

MODE=""; REPO="$(pwd)"; DRY=0; SANDBOX=0
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
    # Note: v1 runs workers sequentially under ORCH_MAX_PARALLEL cap for simplicity.
    # Future versions may background workers (&) up to MAX_P and wait.
    for id in "${ISSUES[@]}"; do
      slug="issue-$id"
      branch="feat/${id}-${slug}"
      path="$(orch_worktree_path "$REPO" "$id" "$slug")"
      port=$(( ${ORCH_BASE_PORT:-3900} + idx ))
      echo "worker id=$id branch=$branch wt=$path port=$port"
      if [ "$DRY" = 0 ]; then
        orch_worktree_add "$REPO" "$id" "$slug" "$branch" HEAD || true
        orch_worktree_bootstrap "$REPO" "$path"
        if [ "$FORCE_CLEAN" = 1 ]; then
          orch_worktree_remove "$REPO" "$path"
        fi
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
