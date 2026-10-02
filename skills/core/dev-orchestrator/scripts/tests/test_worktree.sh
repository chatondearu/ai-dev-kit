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
assert_file "$TMP/.orch/worktrees/42-auth-fix/.env" "env copied"

summary
