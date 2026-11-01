#!/usr/bin/env bash
# orch-cleanup.sh — remove stale .orch/worktrees (dry-run by default).
#
#   bash orch-cleanup.sh --repo DIR [--apply] [--dry-run] [WORKTREE_NAME...]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
source "$ROOT/lib/worktree.sh"
REPO="$(pwd)"
NAMES=()
APPLY=0
DRY=1

usage() {
  cat <<'EOF'
orch-cleanup.sh — prune leftover orch worktrees

  --repo DIR     git repo root (default: cwd)
  --dry-run      list only (default)
  --apply        actually remove worktrees via git worktree remove --force
  WORKTREE_NAME  only these entries of .orch/worktrees (default: all)
  -h|--help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift;;
    --dry-run) DRY=1; APPLY=0;;
    --apply) APPLY=1; DRY=0;;
    -h|--help) usage; exit 0;;
    -*) echo "unknown: $1" >&2; usage >&2; exit 2;;
    *) NAMES+=("$1");;
  esac
  shift
done

REPO="$(orch_repo_root "$REPO")"
WT_ROOT="$REPO/.orch/worktrees"
if [ ! -d "$WT_ROOT" ]; then
  echo "orch-cleanup: no $WT_ROOT"
  exit 0
fi

removed=0
skipped=0
if [ "${#NAMES[@]}" -gt 0 ]; then
  paths=("${NAMES[@]/#/$WT_ROOT/}")
else
  paths=("$WT_ROOT"/*)
fi
for path in "${paths[@]}"; do
  [ -e "$path" ] || continue
  [ -d "$path" ] || continue
  name="$(basename "$path")"
  if [ ! -e "$path/.git" ] && [ ! -f "$path/.git" ]; then
    echo "skip (not a git worktree): $name"
    skipped=$((skipped + 1))
    continue
  fi
  if [ "$DRY" = 1 ]; then
    echo "dry-run: would remove $path"
    removed=$((removed + 1))
    continue
  fi
  if git -C "$REPO" worktree remove --force "$path" 2>/dev/null; then
    echo "removed $path"
    removed=$((removed + 1))
  else
    echo "warn: git worktree remove failed for $path; leaving in place" >&2
    skipped=$((skipped + 1))
  fi
done

echo "orch-cleanup: candidates=$removed skipped=$skipped apply=$APPLY dry-run=$DRY"
