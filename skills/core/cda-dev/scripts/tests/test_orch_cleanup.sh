#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email t@t
git -C "$TMP" config user.name t
echo x >"$TMP/f"
git -C "$TMP" add f
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main

mkdir -p "$TMP/.orch/worktrees"
git -C "$TMP" worktree add -b feat/cleanup-test "$TMP/.orch/worktrees/1-issue-1" main >/dev/null

out="$(bash "$TROOT/../orch-cleanup.sh" --repo "$TMP" --dry-run)"
assert_contains "$out" "dry-run: would remove" "dry-run lists worktree"
assert_file "$TMP/.orch/worktrees/1-issue-1/f" "dry-run did not delete"

bash "$TROOT/../orch-cleanup.sh" --repo "$TMP" --apply >/dev/null
if [ -e "$TMP/.orch/worktrees/1-issue-1" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL worktree still present after --apply\n' >&2
else
  PASSES=$((PASSES + 1))
fi

summary
