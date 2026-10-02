#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(mktemp -d)"
_ORIG_PATH="$PATH"
STUB_BIN="$(mktemp -d)"

cleanup() {
  PATH="$_ORIG_PATH"
  rm -rf "$TMP/.orch/worktrees" 2>/dev/null || true
  git -C "$TMP" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email smoke@test
git -C "$TMP" config user.name smoke
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main

# Setup stub agent that exits 0
cat >"$STUB_BIN/stub-agent" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$STUB_BIN/stub-agent"

# Setup stub npm so real verify ("npm test") succeeds
cat >"$STUB_BIN/npm" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$STUB_BIN/npm"

# Setup configurable stub gh
cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"issue view"*)
    echo "Test Issue"
    ;;
  *"pr list"*)
    case "$*" in
      *"feat/7-"*)
        echo "[]"
        ;;
      *"feat/8-"*)
        echo '[{"number":108}]'
        ;;
      *)
        if [ "${GH_STUB_NO_PR:-0}" = "1" ]; then
          echo "[]"
        elif [ -n "${GH_STUB_PR_NUM:-}" ]; then
          echo "[{\"number\":${GH_STUB_PR_NUM}}]"
        else
          echo "[]"
        fi
        ;;
    esac
    ;;
  *"pr checks"*)
    if [ "${GH_STUB_CHECKS_FAIL:-0}" = "1" ]; then
      echo '[{"name":"ci","state":"FAILURE","bucket":"fail"}]'
    else
      echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'
    fi
    ;;
  *"pr view"*)
    mergeable="${GH_STUB_MERGEABLE:-MERGEABLE}"
    state="${GH_STUB_STATE:-OPEN}"
    echo "{\"mergeable\":\"${mergeable}\",\"state\":\"${state}\",\"headRefName\":\"main\"}"
    ;;
  *)
    exit 0
    ;;
esac
EOF
chmod +x "$STUB_BIN/gh"

export PATH="$STUB_BIN:$PATH"
export ORCH_AGENT="$STUB_BIN/stub-agent"

echo "# run-tests.sh"
bash "$TROOT/run-tests.sh"

echo "# orch dry-run queues all issues beyond MAX_P (issues 1, 2, 3 with MAX_P=2)"
out_dry="$(bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 1 --issue 2 --issue 3 --max-parallel 2 --dry-run)"
assert_contains "$out_dry" "worker id=1 branch=feat/1-issue-1" "dry-run plans issue 1"
assert_contains "$out_dry" "worker id=2 branch=feat/2-issue-2" "dry-run plans issue 2"
assert_contains "$out_dry" "worker id=3 branch=feat/3-issue-3" "dry-run plans issue 3 without dropping"

echo "# orch ready-pickup (issue 1: loop succeeds, no PR found -> status blocked, exit nonzero)"
set +e
out_no_pr="$(GH_STUB_NO_PR=1 bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 1 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_no_pr=$?
set -e
assert_eq "$rc_no_pr" "1" "ready-pickup exits nonzero when worker blocked (no PR)"
assert_contains "$out_no_pr" "blocked: issue 1 (no PR for branch feat/1-issue-1)" "status is blocked with no PR reason"
if echo "$out_no_pr" | grep -q "^done: issue 1"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL done must not be reported without PR\n' >&2
else
  PASSES=$((PASSES + 1))
fi

WT_PATH="$(orch_worktree_path "$TMP" 1 issue-1)"
assert_file "$WT_PATH" "worktree path after dispatch (issue 1)"
assert_file "$WT_PATH/.orch/worker-prompt.md" "rendered worker prompt exists under worktree (issue 1)"

orch_worktree_remove "$TMP" "$WT_PATH"
if [ -e "$TMP/.orch/worktrees/1-issue-1" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL worktree removed: %s/.orch/worktrees/1-issue-1\n' "$TMP" >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# orch ready-pickup (issue 2: PR open + green checks + mergeable -> done, exit 0)"
printf '{"scripts":{"test":"true"}}\n' >"$TMP/package.json"
git -C "$TMP" add package.json
git -C "$TMP" commit -qm "add package.json"

set +e
out_pr_ok="$(GH_STUB_PR_NUM=102 GH_STUB_MERGEABLE=MERGEABLE GH_STUB_STATE=OPEN \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 2 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_pr_ok=$?
set -e
assert_eq "$rc_pr_ok" "0" "ready-pickup exits 0 when PR has green checks and mergeable"
assert_contains "$out_pr_ok" "done: issue 2 (PR 102 checks green, mergeable=MERGEABLE)" "done printed on green PR"

WT_PATH2="$(orch_worktree_path "$TMP" 2 issue-2)"
assert_file "$WT_PATH2" "worktree path after dispatch (issue 2)"
assert_file "$WT_PATH2/.orch/worker-prompt.md" "rendered worker prompt exists under worktree (issue 2)"

orch_worktree_remove "$TMP" "$WT_PATH2"
if [ -e "$TMP/.orch/worktrees/2-issue-2" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL worktree removed: %s/.orch/worktrees/2-issue-2\n' "$TMP" >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# orch ready-pickup (issue 3: checks green but mergeable=CONFLICTING -> blocked, exit nonzero)"
set +e
out_conf="$(GH_STUB_PR_NUM=103 GH_STUB_MERGEABLE=CONFLICTING GH_STUB_STATE=OPEN \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 3 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_conf=$?
set -e
assert_eq "$rc_conf" "1" "orch exits nonzero when PR mergeable fails"
assert_contains "$out_conf" "blocked: PR 103 is not mergeable (mergeable=CONFLICTING)" "blocked reason includes mergeable failure"
if echo "$out_conf" | grep -q "^done: issue 3"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL done must not be reported when mergeable fails\n' >&2
else
  PASSES=$((PASSES + 1))
fi
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 3 issue-3)" 2>/dev/null || true

echo "# orch ready-pickup (issue 4: checks green but state=CLOSED -> blocked, exit nonzero)"
set +e
out_closed="$(GH_STUB_PR_NUM=104 GH_STUB_MERGEABLE=MERGEABLE GH_STUB_STATE=CLOSED \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 4 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_closed=$?
set -e
assert_eq "$rc_closed" "1" "orch exits nonzero when PR state is CLOSED"
assert_contains "$out_closed" "blocked: PR 104 is not mergeable (mergeable=MERGEABLE)" "blocked when PR state is CLOSED"
if echo "$out_closed" | grep -q "^done: issue 4"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL done must not be reported when PR is closed\n' >&2
else
  PASSES=$((PASSES + 1))
fi
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 4 issue-4)" 2>/dev/null || true

echo "# orch ready-pickup multiple issues (issues 5, 6 with MAX_P=1 queues both sequentially)"
set +e
out_multi="$(GH_STUB_PR_NUM=105 GH_STUB_MERGEABLE=MERGEABLE GH_STUB_STATE=OPEN \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 5 --issue 6 --max-parallel 1 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_multi=$?
set -e
assert_eq "$rc_multi" "0" "orch exits 0 when all queued issues pass"
assert_contains "$out_multi" "worker id=5 branch=feat/5-issue-5" "issue 5 dispatched"
assert_contains "$out_multi" "worker id=6 branch=feat/6-issue-6" "issue 6 dispatched beyond MAX_P"
assert_contains "$out_multi" "done: issue 5" "done printed for issue 5"
assert_contains "$out_multi" "done: issue 6" "done printed for issue 6"
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 5 issue-5)" 2>/dev/null || true
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 6 issue-6)" 2>/dev/null || true

echo "# orch ready-pickup (issue 7 blocked, issue 8 done -> overall exit nonzero)"
set +e
out_partial="$(GH_STUB_MERGEABLE=MERGEABLE GH_STUB_STATE=OPEN \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 7 --issue 8 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_partial=$?
set -e
assert_eq "$rc_partial" "1" "overall exit is nonzero when any worker is blocked"
assert_contains "$out_partial" "blocked: issue 7 (no PR for branch feat/7-issue-7)" "issue 7 is blocked"
assert_contains "$out_partial" "done: issue 8 (PR 108 checks green, mergeable=MERGEABLE)" "issue 8 is done"
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 7 issue-7)" 2>/dev/null || true
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 8 issue-8)" 2>/dev/null || true

summary
