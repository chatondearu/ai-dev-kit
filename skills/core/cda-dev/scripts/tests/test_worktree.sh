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
gd="$(git -C "$TMP/.orch/worktrees/42-auth-fix" rev-parse --absolute-git-dir)"
assert_file "$gd/cdamux-managed" "bootstrap marks the worktree as orch-created (worker clean)"
assert_eq "$(git -C "$TMP/.orch/worktrees/42-auth-fix" status --porcelain -- . ':!.env')" "" "marker invisible to git status"
orch_worktree_mark "$TMP"
assert_eq "$([ -e "$TMP/.git/cdamux-managed" ] && echo marked || echo no)" "no" "main checkout never marked"

# orch_free_build_dirs adopts pre-existing worktrees under .orch/worktrees, then cleans.
rm -f "$gd/cdamux-managed"
CBIN="$(mktemp -d)"
printf '#!/bin/sh
echo "$*" >>"%s/cdamux.log"
' "$CBIN" >"$CBIN/cdamux"
chmod +x "$CBIN/cdamux"
PATH="$CBIN:$PATH" orch_free_build_dirs "$TMP"
assert_file "$gd/cdamux-managed" "free_build_dirs adopts an unmarked orch worktree"
assert_eq "$(cat "$CBIN/cdamux.log")" "worker clean --root $TMP/.orch/worktrees" "free_build_dirs runs worker clean on .orch/worktrees"
rm -rf "$CBIN"

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

# Disk guard (ADR-017): 0 = off; an impossible threshold refuses; missing dir walks up.
ORCH_MIN_FREE_GIB=0 orch_disk_guard "$TMP/nope/deeper"
assert_eq "$?" "0" "disk guard off with ORCH_MIN_FREE_GIB=0"
if ORCH_MIN_FREE_GIB=999999999 orch_disk_guard "$TMP/nope" 2>"$TMP/guard.err"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL disk guard must refuse below the threshold\n' >&2
else
  PASSES=$((PASSES + 1))
  printf 'ok   disk guard refuses below the threshold\n'
fi
assert_contains "$(cat "$TMP/guard.err")" "cdamux worker clean" "guard names the clean command"

# L8: unreadable df fails closed; the checkout about to be created counts.
DFBIN="$(mktemp -d)"
printf '#!/bin/sh\nexit 1\n' >"$DFBIN/df"
chmod +x "$DFBIN/df"
if PATH="$DFBIN:$PATH" ORCH_MIN_FREE_GIB=1 orch_disk_guard "$TMP" 2>"$TMP/guard.err"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL disk guard must refuse when df fails\n' >&2
else
  PASSES=$((PASSES + 1))
  printf 'ok   disk guard fails closed on df error\n'
fi
assert_contains "$(cat "$TMP/guard.err")" "cannot read free space" "df error message is explicit"
# 1 GiB + 512 KiB free: enough without the repo, not with a 2 MiB checkout.
printf '#!/bin/sh\necho "Filesystem 1024-blocks Used Available Capacity Mounted"\necho "/dev/x 9999999 1 1049088 1%% /"\n' >"$DFBIN/df"
head -c 2097152 /dev/zero >"$TMP/big"
git -C "$TMP" add big && git -C "$TMP" commit -qm big
PATH="$DFBIN:$PATH" ORCH_MIN_FREE_GIB=1 orch_disk_guard "$TMP"
assert_eq "$?" "0" "guard passes on free space alone"
if PATH="$DFBIN:$PATH" ORCH_MIN_FREE_GIB=1 orch_disk_guard "$TMP" "$TMP" 2>"$TMP/guard.err"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL disk guard must count the checkout\n' >&2
else
  PASSES=$((PASSES + 1))
  printf 'ok   disk guard counts the checkout\n'
fi
assert_contains "$(cat "$TMP/guard.err")" "MiB for the checkout" "message names the checkout size"
rm -rf "$DFBIN"

summary
