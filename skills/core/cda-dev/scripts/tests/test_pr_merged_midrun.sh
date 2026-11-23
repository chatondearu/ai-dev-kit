#!/usr/bin/env bash
# #374: merged mid-run short-circuit + one open-PR step when missing.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/ci.sh
source "$TROOT/../lib/ci.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(cd "$(mktemp -d)" && pwd -P)"
_ORIG_PATH="$PATH"
STUB_BIN="$(mktemp -d)"
STATE_FILE="$STUB_BIN/pr-state"
OPENED_MARKER="$STUB_BIN/opened"

cleanup() {
  PATH="$_ORIG_PATH"
  rm -rf "$TMP/.orch/worktrees" 2>/dev/null || true
  git -C "$TMP" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email test@test
git -C "$TMP" config user.name test
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main
printf '{"scripts":{"test":"true"}}\n' >"$TMP/package.json"
git -C "$TMP" add package.json
git -C "$TMP" commit -qm "verify"

echo "# orch_pr_state unit (stub gh)"
cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"pr list"*"--state all"*)
    st="$(cat "${GH_PR_STATE_FILE:-/dev/null}" 2>/dev/null || true)"
    case "$st" in
      MERGED) echo '[{"number":9,"state":"MERGED"}]' ;;
      CLOSED) echo '[{"number":9,"state":"CLOSED"}]' ;;
      OPEN) echo '[{"number":9,"state":"OPEN"}]' ;;
      BOTH) echo '[{"number":8,"state":"CLOSED"},{"number":9,"state":"OPEN"}]' ;;
      ERR) exit 1 ;;
      *) echo '[]' ;;
    esac
    ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"
export GH_PR_STATE_FILE="$STATE_FILE"

printf 'OPEN\n' >"$STATE_FILE"
assert_eq "$(orch_pr_state feat/x)" "OPEN" "orch_pr_state OPEN"
printf 'MERGED\n' >"$STATE_FILE"
assert_eq "$(orch_pr_state feat/x)" "MERGED" "orch_pr_state MERGED"
printf 'CLOSED\n' >"$STATE_FILE"
assert_eq "$(orch_pr_state feat/x)" "CLOSED" "orch_pr_state CLOSED"
: >"$STATE_FILE"
assert_eq "$(orch_pr_state feat/x)" "" "orch_pr_state empty when no PR"
printf 'ERR\n' >"$STATE_FILE"
assert_eq "$(orch_pr_state feat/x || true)" "UNKNOWN" "orch_pr_state UNKNOWN on gh error, never no-PR (L11)"

printf 'OPEN\n' >"$STATE_FILE"
got="$(orch_pr_list_open_or_merged feat/x)"
assert_contains "$got" '"number":9' "list keeps OPEN"
printf 'CLOSED\n' >"$STATE_FILE"
got="$(orch_pr_list_open_or_merged feat/x)"
assert_eq "$got" "[]" "list drops CLOSED-only"
printf 'BOTH\n' >"$STATE_FILE"
got="$(orch_pr_list_open_or_merged feat/x)"
assert_contains "$got" '"number":9' "list picks OPEN after CLOSED"
assert_eq "$(echo "$got" | jq 'length')" "1" "list filters to one usable PR"

echo "# no-jq: CLOSED-only → [] (same as jq path)"
# Nix puts jq next to sed/grep — symlink tools into a bin that omits jq.
_no_jq_bin="$(mktemp -d)"
for _b in bash sed grep printf head cut cat tr mktemp; do
  _p="$(command -v "$_b" 2>/dev/null)" || continue
  ln -s "$_p" "$_no_jq_bin/$_b"
done
ln -s "$STUB_BIN/gh" "$_no_jq_bin/gh"
PATH="$_no_jq_bin"
if command -v jq >/dev/null 2>&1; then
  FAILS=$((FAILS + 1))
  printf 'FAIL no-jq PATH still finds jq\n' >&2
else
  PASSES=$((PASSES + 1))
fi
printf 'CLOSED\n' >"$STATE_FILE"
got="$(orch_pr_list_open_or_merged feat/x)"
assert_eq "$got" "[]" "no-jq list drops CLOSED-only"
printf 'BOTH\n' >"$STATE_FILE"
got="$(orch_pr_list_open_or_merged feat/x)"
assert_contains "$got" '"number":9' "no-jq list picks OPEN after CLOSED"
if echo "$got" | grep -q '"state":"CLOSED"'; then
  FAILS=$((FAILS + 1))
  printf 'FAIL no-jq must filter CLOSED entries\n' >&2
else
  PASSES=$((PASSES + 1))
fi
printf 'OPEN\n' >"$STATE_FILE"
got="$(orch_pr_list_open_or_merged feat/x)"
assert_contains "$got" '"number":9' "no-jq list keeps OPEN"
PATH="$STUB_BIN:$_ORIG_PATH"
rm -rf "$_no_jq_bin"
unset _no_jq_bin _b _p

echo "# ready-pickup integration stubs"
cat >"$STUB_BIN/stub-agent" <<EOF
#!/usr/bin/env bash
prompt="\$(cat)"
mkdir -p .orch
case "\$prompt" in
  *"Verifier (grounding node)"*) touch .orch/verifier-ok ;;
  *"Reviewer prompt"*) mkdir -p .orch; echo approved >.orch/review-verdict ;;
  *"Commit all changes"*) touch "$OPENED_MARKER" ;;
esac
exit 0
EOF
chmod +x "$STUB_BIN/stub-agent"

cat >"$STUB_BIN/gh" <<EOF
#!/usr/bin/env bash
case "\$*" in
  *"issue view"*)
    case "\$*" in
      *"--comments"*|*"--json comments"*)
        printf '%s\\n' '{"comments":[{"authorAssociation":"OWNER","body":"## Agent Brief\\n\\n**Acceptance criteria:**\\n- [ ] done\\n"}]}'
        ;;
      *"--json body"*) printf '%s\\n' '{"body":"Test"}' ;;
      *"--json title"*) echo "Test" ;;
      *) echo "Test" ;;
    esac
    ;;
  *"pr list"*)
    st="\$(cat "$STATE_FILE" 2>/dev/null || true)"
    case "\$st" in
      MERGED)
        echo '[{"number":201,"url":"https://example/pull/201","state":"MERGED","title":"x"}]'
        ;;
      OPEN_THEN_MERGED_AT_REVIEW)
        # orch_pr_state uses --json number,state; list uses url,state,title.
        # Probe order: graph + post-loop + line-399 = 3 OPEN, then round-1 MERGED.
        # Distinguish orch_pr_state (number,state) from list (number,url,state,title).
        case "\$*" in
          *"--json number,state"*)
            n=0
            [ -f "$STUB_BIN/state-calls" ] && n="\$(cat "$STUB_BIN/state-calls")"
            n=\$((n + 1))
            echo "\$n" >"$STUB_BIN/state-calls"
            if [ "\$n" -ge 4 ]; then
              echo '[{"number":203,"state":"MERGED"}]'
            else
              echo '[{"number":203,"state":"OPEN"}]'
            fi
            ;;
          *)
            echo '[{"number":203,"url":"https://example/pull/203","state":"OPEN","title":"x"}]'
            ;;
        esac
        ;;
      OPEN_AFTER_OPEN)
        if [ -f "$OPENED_MARKER" ]; then
          echo '[{"number":202,"url":"https://example/pull/202","state":"OPEN","title":"x"}]'
        else
          echo '[]'
        fi
        ;;
      *)
        echo '[]'
        ;;
    esac
    ;;
  *"pr checks"*)
    echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'
    ;;
  *"pr view"*)
    echo '{"mergeable":"MERGEABLE","state":"OPEN","headRefName":"feat/x"}'
    ;;
  *"repo view"*)
    case "\$*" in *"-q"*) echo "stub/repo" ;; *) echo '{"nameWithOwner":"stub/repo"}' ;; esac
    ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
cat >"$STUB_BIN/npm" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$STUB_BIN/npm"

export ORCH_AGENT="$STUB_BIN/stub-agent"
export ORCH_PROGRESS=0
export ORCH_TMUX=0

echo "# ready-pickup: MERGED at lookup → done (no review/CI)"
printf 'MERGED\n' >"$STATE_FILE"
rm -f "$OPENED_MARKER"
set +e
out_merged="$(bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 40 \
  --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_merged=$?
set -e
assert_eq "$rc_merged" "0" "merged mid-run exits 0"
assert_contains "$out_merged" "done: issue 40 (merged by maintainer)" "merged reason printed"
if echo "$out_merged" | grep -q "reviewer round"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL merged path must skip review\n' >&2
else
  PASSES=$((PASSES + 1))
fi
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 40 issue-40)" 2>/dev/null || true

echo "# ready-pickup: OPEN at lookup, MERGED before review round 1 → done (no reviewer)"
printf 'OPEN_THEN_MERGED_AT_REVIEW\n' >"$STATE_FILE"
rm -f "$OPENED_MARKER" "$STUB_BIN/state-calls"
set +e
out_rev="$(bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 44 \
  --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_rev=$?
set -e
assert_eq "$rc_rev" "0" "merged during review rounds exits 0"
assert_contains "$out_rev" "done: issue 44 (merged by maintainer)" "merged during review reason"
if echo "$out_rev" | grep -q "reviewer round"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL review-round short-circuit must skip reviewer\n' >&2
else
  PASSES=$((PASSES + 1))
fi
if echo "$out_rev" | grep -qi "review fix"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL review-round short-circuit must not fix-push\n' >&2
else
  PASSES=$((PASSES + 1))
fi
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 44 issue-44)" 2>/dev/null || true

echo "# ready-pickup: no PR → one open-PR step → then PR appears → done"
printf 'OPEN_AFTER_OPEN\n' >"$STATE_FILE"
rm -f "$OPENED_MARKER"
set +e
out_open="$(bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 41 \
  --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_open=$?
set -e
assert_eq "$rc_open" "0" "open-PR step then green exits 0"
assert_file "$OPENED_MARKER" "open-PR agent prompt was run"
assert_contains "$out_open" "done: issue 41 (PR 202 checks green, mergeable=MERGEABLE)" "done after open-PR step"
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 41 issue-41)" 2>/dev/null || true

echo "# ready-pickup: no PR and open-PR step still yields none → blocked"
printf 'NONE\n' >"$STATE_FILE"
rm -f "$OPENED_MARKER"
set +e
out_none="$(bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 42 \
  --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_none=$?
set -e
assert_eq "$rc_none" "1" "still blocked when open-PR yields nothing"
assert_file "$OPENED_MARKER" "open-PR attempted before blocked"
assert_contains "$out_none" "blocked: issue 42 (no PR for branch feat/42-issue-42)" "blocked reason unchanged"
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 42 issue-42)" 2>/dev/null || true

echo "# graph loop: MERGED before iter → return 0 without agent"
# shellcheck source=../lib/verify.sh
source "$TROOT/../lib/verify.sh"
# shellcheck source=../lib/agents.sh
source "$TROOT/../lib/agents.sh"
# shellcheck source=../lib/progress.sh
source "$TROOT/../lib/progress.sh"
# shellcheck source=../lib/graph.sh
source "$TROOT/../lib/graph.sh"
# shellcheck source=../lib/loop.sh
source "$TROOT/../lib/loop.sh"

# gh stub for orch_pr_state during graph
cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"pr list"*"--state all"*)
    echo '[{"number":1,"state":"MERGED"}]'
    ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"

WTG="$(mktemp -d)"
mkdir -p "$WTG/.orch"
export ORCH_WORKER_BRANCH="feat/graph-merged"
export ORCH_WORKER_ID="99"
export ORCH_PROGRESS=0
export ORCH_GRAPH=0
export ORCH_GRAPH_VERIFY=0
export ORCH_VERIFY=true
PROMPT="$WTG/.orch/p.md"
echo "implement" >"$PROMPT"
AGENT_HITS="$STUB_BIN/agent-hits"
rm -f "$AGENT_HITS"
cat >"$STUB_BIN/count-agent" <<EOF
#!/usr/bin/env bash
echo hit >>"$AGENT_HITS"
cat >/dev/null
exit 0
EOF
chmod +x "$STUB_BIN/count-agent"
export ORCH_AGENT="$STUB_BIN/count-agent"
set +e
orch_graph_worker "$WTG" "$PROMPT" >/dev/null 2>&1
rc_graph=$?
set -e
assert_eq "$rc_graph" "0" "graph returns 0 when already MERGED"
if [ -f "$AGENT_HITS" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL graph must not run agent when MERGED\n' >&2
else
  PASSES=$((PASSES + 1))
fi
rm -rf "$WTG"

summary
