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
if [ -e "$TMP/.orch/worktrees/42-auth-fix/.env" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL .env must not copy by default\n' >&2
else
  PASSES=$((PASSES + 1))
  printf 'ok   .env not copied by default\n'
fi
ORCH_COPY_ENV=1 orch_worktree_bootstrap "$TMP" "$TMP/.orch/worktrees/42-auth-fix"
assert_file "$TMP/.orch/worktrees/42-auth-fix/.env" "env copied with ORCH_COPY_ENV=1"

# Reuse existing worktree on retry
set +e
orch_worktree_add "$TMP" 42 auth-fix "feat/42-auth-fix" main
reuse_rc=$?
set -e
assert_eq "$reuse_rc" "0" "reuse existing worktree returns 0"
branch2="$(git -C "$TMP/.orch/worktrees/42-auth-fix" branch --show-current)"
assert_eq "$branch2" "feat/42-auth-fix" "reuse keeps branch"
# Detached HEAD (worker blocked mid-rebase) with uncommitted work: never deleted
wt="$TMP/.orch/worktrees/42-auth-fix"
git -C "$wt" checkout -q --detach
echo precious >"$wt/uncommitted.txt"
set +e
orch_worktree_add "$TMP" 42 auth-fix "feat/42-auth-fix" main 2>/dev/null
detached_rc=$?
set -e
assert_eq "$detached_rc" "1" "detached HEAD worktree is not replaced"
assert_eq "$(cat "$wt/uncommitted.txt")" "precious" "uncommitted file survives a detached-HEAD reuse attempt"

# Other branch + uncommitted file: also kept
git -C "$wt" checkout -q -b other
set +e
orch_worktree_add "$TMP" 42 auth-fix "feat/42-auth-fix" main 2>/dev/null
dirty_rc=$?
set -e
assert_eq "$dirty_rc" "1" "dirty worktree on another branch is not replaced"
assert_file "$wt/uncommitted.txt" "uncommitted file survives a dirty replace attempt"

# Other branch, clean tree: replaced without --force
rm "$wt/uncommitted.txt" "$wt/.env"
orch_worktree_add "$TMP" 42 auth-fix "feat/42-auth-fix" main 2>/dev/null
assert_eq "$(git -C "$wt" branch --show-current)" "feat/42-auth-fix" "clean worktree on another branch is replaced"

# Dir deleted by hand but still registered: next add must succeed (prune first)
rm -rf "$wt"
orch_worktree_add "$TMP" 42 auth-fix "feat/42-auth-fix" main 2>/dev/null
assert_file "$wt/f" "worktree re-added after its dir was deleted"
assert_eq "$(git -C "$wt" branch --show-current)" "feat/42-auth-fix" "re-added worktree on the branch"

summary
