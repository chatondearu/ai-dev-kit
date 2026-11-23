#!/usr/bin/env bash
# merge-lock.sh --repo DIR merge PR     merge PR by hand under the merge lock (below)
# merge-lock.sh --repo DIR -- CMD...    run CMD in DIR under the merge lock
# The lock is DIR's .orch/merge.lock, the one orch.sh holds for its own merges (Q4).
# `merge` refuses unless the PR head contains origin/<base> after a fetch (behind →
# the worker rebases, re-runs verify, pushes), the repo allows rebase merges and
# mergeable is MERGEABLE; then `gh pr merge --rebase --match-head-commit <fetched head>`.
# Exit: CMD's status / 0 merged, 1 refused, 75 lock still held (ORCH_MERGE_LOCK_WAIT).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
source "$ROOT/lib/worktree.sh"
source "$ROOT/lib/lock.sh"
source "$ROOT/lib/ci.sh"

REPO="$(pwd)"
MERGE_PR=""
while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    merge) MERGE_PR="${2:-}"; shift 2 || shift; break ;;
    --) shift; break ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "merge-lock: unknown option $1" >&2; exit 2 ;;
  esac
done
if [ -z "$MERGE_PR" ] && [ $# -eq 0 ]; then
  echo "merge-lock: need 'merge <pr>' or -- CMD" >&2
  exit 2
fi
ORCH_REPO="$(orch_repo_root "$REPO")"
cd "$ORCH_REPO"
# gh resolves the repo from DIR, never an inherited GH_REPO.
export GH_REPO=""

refuse() { printf 'merge-lock: PR #%s not merged: %s\n' "$MERGE_PR" "$1" >&2; return 1; }

merge_pr() {
  local pr="$1" head base oid allowed m
  head="$(gh pr view "$pr" --json headRefName -q .headRefName 2>/dev/null)" || head=""
  base="$(gh pr view "$pr" --json baseRefName -q .baseRefName 2>/dev/null)" || base=""
  [ -n "$head" ] && [ -n "$base" ] || { refuse "PR metadata unreadable"; return 1; }
  [ "$head" != "$base" ] || { refuse "head is the base branch $base"; return 1; }
  allowed="$(gh repo view --json rebaseMergeAllowed -q .rebaseMergeAllowed 2>/dev/null)" || allowed=""
  [ "$allowed" = true ] || { refuse "the repo does not allow rebase merges (rebaseMergeAllowed=${allowed:-unreadable})"; return 1; }
  git fetch -q origin "$base" "$head" 9>&- || { refuse "git fetch origin $base $head failed"; return 1; }
  oid="$(git rev-parse "origin/$head")" || { refuse "cannot read origin/$head"; return 1; }
  git merge-base --is-ancestor "origin/$base" "$oid" || {
    refuse "behind origin/$base: the worker rebases on it, re-runs verify and pushes; then merge again"
    return 1
  }
  m="$(orch_pr_mergeable "$pr" 9>&-)" || { refuse "not mergeable (mergeable=${m:-UNKNOWN})"; return 1; }
  GH_PROMPT_DISABLED=1 gh pr merge "$pr" --rebase --match-head-commit "$oid" </dev/null 9>&- \
    || { refuse "gh pr merge --rebase failed"; return 1; }
  printf 'merge-lock: merged PR #%s (%s at %s, --rebase)\n' "$pr" "$head" "$oid" >&2
}

if ! orch_merge_lock_acquire; then
  printf 'merge-lock: %s/.orch/merge.lock still held (ORCH_MERGE_LOCK_WAIT)\n' "$ORCH_REPO" >&2
  exit 75
fi
rc=0
if [ -n "$MERGE_PR" ]; then
  merge_pr "$MERGE_PR" || rc=1
else
  "$@" 9>&- || rc=$?
fi
orch_merge_lock_release
exit "$rc"
