#!/usr/bin/env bash
# Merge freshness (Q4/Q5/Q11, audit C2/H6): default branch resolved (here `develop`,
# never assumed main), worktrees cut from origin/<default>, merge under the per-repo
# lock after fetch + rebase + re-verify, merge-after-sibling-merge with a gh stub that
# really rebase-merges into the bare origin.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
source "$TROOT/../lib/worktree.sh"
source "$TROOT/../lib/lock.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/ci.sh"
source "$TROOT/../lib/meta.sh"

TMP="$(cd "$(mktemp -d)" && pwd -P)"
_ORIG_PATH="$PATH"
cleanup() {
  PATH="$_ORIG_PATH"
  git -C "$TMP/repo" worktree prune 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
ORIGIN="$TMP/origin.git"
R="$TMP/repo"
SIB="$TMP/sibling"
STUB="$TMP/stub"
mkdir -p "$STUB"

git init -q --bare -b develop "$ORIGIN"
git init -q -b develop "$R"
echo init >"$R/README"
git -C "$R" add README
git -C "$R" commit -qm init
git -C "$R" remote add origin "$ORIGIN"
git -C "$R" push -q origin develop
git clone -q "$ORIGIN" "$SIB"

# A sibling PR lands on origin/develop (the local checkout never pulls it).
sibling_push() { # file content
  git -C "$SIB" pull -q --rebase origin develop
  printf '%s\n' "$2" >"$SIB/$1"
  git -C "$SIB" add "$1"
  git -C "$SIB" commit -qm "sibling $1"
  git -C "$SIB" push -q origin develop
}

# gh stub: PR <n> → branch in $STUB/pr-<n>. `pr merge --rebase --match-head-commit SHA`
# refuses a different remote head, then rebases the branch onto origin/develop and
# pushes it there, like GitHub. Overlapping merges are recorded in $STUB/overlap.
cat >"$STUB/gh" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$STUB/gh.log"
case "\$*" in
  *rebaseMergeAllowed*) echo true ;;
  *"pr checks"*) echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]' ;;
  *"pr view"*) echo '{"mergeable":"MERGEABLE","state":"OPEN"}' ;;
  *"pr merge"*)
    set -e
    pr="\$3"; br="\$(cat "$STUB/pr-\$pr")"; sha="\${@: -1}"
    mkdir "$STUB/merging" 2>/dev/null || echo "\$pr" >>"$STUB/overlap"
    sleep 1
    gc="$STUB/gh-clone-\$pr"
    rm -rf "\$gc"; git clone -q "$ORIGIN" "\$gc"
    [ "\$(git -C "\$gc" rev-parse "origin/\$br")" = "\$sha" ] || { echo "head moved \$(git -C "\$gc" rev-parse "origin/\$br") != \$sha" >&2; rmdir "$STUB/merging"; exit 1; }
    git -C "\$gc" checkout -q -B tmp "origin/\$br"
    git -C "\$gc" rebase -q origin/develop
    git -C "\$gc" push -q origin tmp:develop
    rmdir "$STUB/merging" 2>/dev/null || true
    ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB/gh"
export PATH="$STUB:$PATH" ORCH_REPO="$R" ORCH_MERGEABLE_WAIT=0 ORCH_PROGRESS=0

echo "# prompts never hardcode main (Q4): worker, reviewer, verifier use {{DEFAULT_BRANCH}}"
assert_eq "$(grep -l 'origin/main\|fetch origin main' "$TROOT"/../../prompts/{worker,reviewer,verifier}.md || true)" "" \
  "no origin/main in agent prompts"

echo "# default branch resolved from origin, never assumed main"
assert_eq "$(orch_default_branch "$R")" "develop" "origin HEAD → develop"

echo "# stale local checkout: worktree is cut from the fetched origin/develop (H6)"
mkdir -p "$SIB/plans"
sibling_push plans/PLAN-7-x.md plan
orch_sync_default "$R"
assert_eq "$ORCH_BASE" "origin/develop" "ORCH_BASE is origin/<default>"
assert_eq "$ORCH_DEFAULT_BRANCH" "develop" "ORCH_DEFAULT_BRANCH exported"
[ ! -e "$R/plans/PLAN-7-x.md" ] && PASSES=$((PASSES + 1)) \
  || { FAILS=$((FAILS + 1)); echo "FAIL local checkout should still be stale" >&2; }

new_branch() { # id branch file
  orch_worktree_add "$R" "$1" x "$2" "$ORCH_BASE" >/dev/null 2>&1
  local wt; wt="$(orch_worktree_path "$R" "$1" x)"
  echo "$1" >"$wt/$3"
  git -C "$wt" add "$3"
  git -C "$wt" commit -qm "$2"
  git -C "$wt" push -q origin "HEAD:$2"
  printf '%s\n' "$wt"
}
WT_A="$(new_branch 1 feat/a a.txt)"
assert_eq "$(git -C "$WT_A" rev-parse --abbrev-ref @{upstream} 2>/dev/null || echo none)" "none" \
  "feature branch does not track origin/<default>"
assert_file "$WT_A/plans/PLAN-7-x.md" "plan merged upstream is in the worktree"

echo "# merge after a sibling merge: fetch, rebase, re-verify, push, merge (C2)"
sibling_push b.txt b
echo feat/a >"$STUB/pr-501"
: >"$STUB/gh.log"
set +e
ORCH_VERIFY="echo ran >>'$TMP/verify.log'; test -f b.txt" orch_merge_pr 501 "$WT_A" feat/a 2>"$TMP/m1.err"
rc=$?
set -e
assert_eq "$rc" "0" "behind branch is rebased and merged"
assert_contains "$(cat "$TMP/m1.err")" "behind origin/develop; rebasing" "rebase announced"
assert_eq "$(wc -l <"$TMP/verify.log" | tr -d ' ')" "1" "verify re-ran on the rebased tree"
head_a="$(git -C "$WT_A" rev-parse HEAD)"
assert_eq "$(git -C "$R" ls-remote origin refs/heads/feat/a | cut -f1)" "$head_a" "rebased head pushed to the PR branch"
assert_contains "$(cat "$STUB/gh.log")" "pr merge 501 --rebase --match-head-commit $head_a" "merge pinned to the verified head"
git -C "$R" fetch -q origin develop
git -C "$R" cat-file -e origin/develop:a.txt && git -C "$R" cat-file -e origin/develop:b.txt \
  && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL develop must hold both PRs" >&2; }

echo "# verify red after the rebase: no push, no merge"
WT_C="$(new_branch 3 feat/c c.txt)"
c_before="$(git -C "$R" ls-remote origin refs/heads/feat/c | cut -f1)"
sibling_push broken x
echo feat/c >"$STUB/pr-503"
: >"$STUB/gh.log"
set +e
ORCH_VERIFY='test ! -e broken' orch_merge_pr 503 "$WT_C" feat/c 2>/dev/null
rc=$?
set -e
assert_eq "$rc" "1" "verify failure after rebase refuses the merge"
assert_contains "$ORCH_MERGE_ERROR" "verify failed after rebase on origin/develop" "reason names the rebase verify"
assert_eq "$(git -C "$R" ls-remote origin refs/heads/feat/c | cut -f1)" "$c_before" "red rebase is not pushed"
grep -q "pr merge" "$STUB/gh.log" && { FAILS=$((FAILS + 1)); echo "FAIL merged a red rebase" >&2; } || PASSES=$((PASSES + 1))

echo "# rebase conflict: aborted, clear error"
WT_D="$(new_branch 4 feat/d d.txt)"
sibling_push d.txt conflicting
echo feat/d >"$STUB/pr-504"
set +e
ORCH_VERIFY=true orch_merge_pr 504 "$WT_D" feat/d 2>/dev/null
rc=$?
set -e
assert_eq "$rc" "1" "conflict refuses the merge"
assert_contains "$ORCH_MERGE_ERROR" "conflicts" "conflict reason"
[ -d "$(git -C "$WT_D" rev-parse --git-path rebase-merge)" ] \
  && { FAILS=$((FAILS + 1)); echo "FAIL rebase left in progress" >&2; } || PASSES=$((PASSES + 1))

echo "# dirty worktree: its own reason, not a conflict"
WT_G="$(new_branch 7 feat/g g.txt)"
sibling_push g2.txt g2
echo dirty >>"$WT_G/g.txt"
set +e
ORCH_VERIFY=true orch_merge_pr 507 "$WT_G" feat/g 2>/dev/null
rc=$?
set -e
assert_eq "$rc" "1" "dirty worktree refuses the merge"
assert_contains "$ORCH_MERGE_ERROR" "uncommitted changes" "dirty reason"

echo "# head branch = default branch is refused"
set +e
orch_merge_pr 505 "$WT_D" develop 2>/dev/null
rc=$?
set -e
assert_eq "$rc" "1" "never merges the default branch onto itself"

# Two sibling PRs merged in parallel: the lock serializes them and the second one
# rebases onto the first (the merge-after-sibling-merge incident).
parallel_merges() { # label
  local wt_e wt_f
  wt_e="$(new_branch "e$1" "feat/e$1" "e$1.txt")"
  wt_f="$(new_branch "f$1" "feat/f$1" "f$1.txt")"
  echo "feat/e$1" >"$STUB/pr-61$1"
  echo "feat/f$1" >"$STUB/pr-62$1"
  rm -f "$STUB/overlap"
  ( ORCH_VERIFY=true orch_merge_pr "61$1" "$wt_e" "feat/e$1" 2>/dev/null; echo $? >"$TMP/rc-e$1" ) &
  ( ORCH_VERIFY=true orch_merge_pr "62$1" "$wt_f" "feat/f$1" 2>/dev/null; echo $? >"$TMP/rc-f$1" ) &
  wait
  assert_eq "$(cat "$TMP/rc-e$1")$(cat "$TMP/rc-f$1")" "00" "$1: both parallel merges succeed"
  [ ! -e "$STUB/overlap" ] && PASSES=$((PASSES + 1)) \
    || { FAILS=$((FAILS + 1)); echo "FAIL $1: merges overlapped (no merge lock)" >&2; }
  git -C "$R" fetch -q origin develop
  git -C "$R" cat-file -e "origin/develop:e$1.txt" && git -C "$R" cat-file -e "origin/develop:f$1.txt" \
    && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL $1: develop must hold both PRs" >&2; }
}
echo "# parallel sibling merges are serialized (merge lock)"
orch_sync_default "$R"
parallel_merges p
if command -v flock >/dev/null 2>&1; then
  echo "# same, mkdir fallback (flock hidden)"
  PATH="$(orch_test_path_without "$TMP" flock)"
  orch_sync_default "$R"
  parallel_merges m
  PATH="$STUB:$_ORIG_PATH"
fi

echo "# merge lock held elsewhere: bounded wait, then a clear error"
mkdir -p "$R/.orch/merge.lock.d"
sleep 30 & holder=$!
echo "$holder" >"$R/.orch/merge.lock.d/pid"
_ORCH_FLOCK_HIDDEN="$(orch_test_path_without "$TMP" flock)"
set +e
PATH="$_ORCH_FLOCK_HIDDEN" ORCH_MERGE_LOCK_WAIT=1 ORCH_VERIFY=true orch_merge_pr 504 "$WT_D" feat/d 2>/dev/null
rc=$?
set -e
kill "$holder" 2>/dev/null || true
rm -rf "$R/.orch/merge.lock.d"
assert_eq "$rc" "1" "held merge lock times out"
assert_contains "$ORCH_MERGE_ERROR" "merge lock" "lock reason"

summary
