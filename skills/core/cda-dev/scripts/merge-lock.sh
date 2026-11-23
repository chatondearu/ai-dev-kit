#!/usr/bin/env bash
# merge-lock.sh --repo DIR -- CMD...
# Run CMD under DIR's per-repo merge lock (.orch/merge.lock), the one orch.sh holds
# for its own merges (Q4). For merges done by hand, e.g.
#   merge-lock.sh --repo ~/dev/kit -- gh pr merge 12 --rebase
# Exits with CMD's status; 75 when the lock stays held past ORCH_MERGE_LOCK_WAIT.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
source "$ROOT/lib/worktree.sh"
source "$ROOT/lib/lock.sh"

REPO="$(pwd)"
while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    --) shift; break ;;
    -h|--help) sed -n '2,6p' "$0"; exit 0 ;;
    *) echo "merge-lock: unknown option $1" >&2; exit 2 ;;
  esac
done
[ $# -gt 0 ] || { echo "merge-lock: need -- CMD" >&2; exit 2; }
ORCH_REPO="$(orch_repo_root "$REPO")"
if ! orch_merge_lock_acquire; then
  printf 'merge-lock: %s/.orch/merge.lock still held (ORCH_MERGE_LOCK_WAIT)\n' "$ORCH_REPO" >&2
  exit 75
fi
rc=0
"$@" 9>&- || rc=$?
orch_merge_lock_release
exit "$rc"
