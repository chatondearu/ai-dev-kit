#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(mktemp -d)"
WT_PATH=""

cleanup() {
  if [ -n "$WT_PATH" ] && [ -e "$WT_PATH" ]; then
    orch_worktree_remove "$TMP" "$WT_PATH" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email smoke@test
git -C "$TMP" config user.name smoke
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main

echo "# run-tests.sh"
bash "$TROOT/run-tests.sh"

echo "# orch dry-run (issues 1, 2)"
bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 1 --issue 2 --dry-run

echo "# orch ready-pickup (issue 1)"
bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 1

WT_PATH="$(orch_worktree_path "$TMP" 1 issue-1)"
assert_file "$WT_PATH" "worktree path after dispatch"

orch_worktree_remove "$TMP" "$WT_PATH"
WT_PATH=""
if [ -e "$TMP/.orch/worktrees/1-issue-1" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL worktree removed: %s/.orch/worktrees/1-issue-1\n' "$TMP" >&2
else
  PASSES=$((PASSES + 1))
fi

summary
