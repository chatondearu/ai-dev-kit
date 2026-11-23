#!/usr/bin/env bash
# #236.1: pr-fix finishes the CDAMUX_HOME run (status + ended_at) on success.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"

TMP="$(mktemp -d)"
STUB_BIN="$(mktemp -d)"
_ORIG_PATH="$PATH"
cleanup() {
  PATH="$_ORIG_PATH"
  git -C "$TMP/repo" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

R="$TMP/repo"
mkdir -p "$R"
git -C "$R" init -q
git -C "$R" config user.email t@t
git -C "$R" config user.name t
echo init >"$R/README"
git -C "$R" add README
git -C "$R" commit -qm init
git -C "$R" checkout -qb feat/x
echo x >>"$R/README"
git -C "$R" add README
git -C "$R" commit -qm x
git -C "$R" checkout -q master 2>/dev/null || git -C "$R" checkout -q main

orch_test_stub_cdamux "$TMP"
export CDAMUX_HOME="$TMP/cdamux" ORCH_GRAPH=1 ORCH_TMUX=0 ORCH_PROGRESS=0
export ORCH_VERIFY=true ORCH_ALLOW_NO_CHECKS=1 ORCH_GRAPH_VERIFY=0
export ORCH_RUN_ID="prfix-finish-1"
export TMPDIR="$TMP/tmpd"; mkdir -p "$TMPDIR"

cat >"$STUB_BIN/stub-agent" <<EOF
#!/usr/bin/env bash
prompt="\$(cat)"
case "\$prompt" in
  *"Reviewer prompt"*)
    echo "\${STUB_REVIEW:-approved}" >"\$(printf '%s\n' "\$prompt" | sed -n 's/^Verdict file: //p' | head -n 1)"
    ls .orch/reviewer-prompt-* >>"$TMP/prompt-in-wt" 2>/dev/null || true
    printf '%s\n' "\$prompt" | sed -n 's/^Verdict file: //p' >>"$TMP/verdict-paths"
    echo x >>"$TMP/reviews"
    ;;
  *) touch "$TMP/agent-ran" ;;
esac
EOF
chmod +x "$STUB_BIN/stub-agent"

# Trusted feedback so early "nothing to fix" does not skip the agent path.
export GH_STUB_PR='{
  "commits":[{"committedDate":"2026-11-10T10:00:00Z"}],
  "reviews":[{"author":{"login":"alice"},"authorAssociation":"OWNER","state":"CHANGES_REQUESTED","submittedAt":"2026-11-10T11:00:00Z","body":"FIX"}],
  "comments":[]
}'
export GH_STUB_TH='{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[]}}}}}'
GREEN='[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'

cat >"$STUB_BIN/gh" <<EOF
#!/usr/bin/env bash
case "\$*" in
  *"pr view"*headRefName*) echo feat/x ;;
  *"pr view"*isCrossRepository*) echo false ;;
  *"repo view"*defaultBranchRef*) echo main ;;
  *"pr view"*reviews*) echo "\$GH_STUB_PR" ;;
  *"api graphql"*) echo "\$GH_STUB_TH" ;;
  *"pr checks"*) echo '$GREEN' ;;
  *"pr view"*mergeable*) echo '{"mergeable":"MERGEABLE","state":"OPEN"}' ;;
  *"pr checkout"*) exit 0 ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$TMP/bin:$PATH"
export ORCH_AGENT="$STUB_BIN/stub-agent"

set +e
out="$(bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr 77 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc=$?
set -e

assert_eq "$rc" "0" "pr-fix exits 0"
assert_file "$TMP/agent-ran" "agent ran"
gf="$CDAMUX_HOME/runs/${ORCH_RUN_ID}.json"
assert_file "$gf" "run written under CDAMUX_HOME"
assert_eq "$(jq -r '.status' "$gf")" "ok" "pr-fix finish sets status ok"
ended="$(jq -r '.ended_at // empty' "$gf")"
if [ -n "$ended" ]; then
  assert_eq "has-ended" "has-ended" "pr-fix finish sets ended_at"
else
  assert_eq "missing-ended" "has-ended" "pr-fix finish sets ended_at"
fi
assert_contains "$out" "done: PR 77" "success message"
assert_eq "$(wc -l <"$TMP/reviews" | tr -d ' ')" "1" "pr-fix runs the independent review gate (M12)"
assert_eq "$(cat "$TMP/prompt-in-wt" 2>/dev/null)" "" "reviewer prompt (naming the verdict file) is not in the worktree"
assert_eq "$(ls "$TMPDIR")" "" "review dir (prompt + verdict) removed after the review"
case "$(cat "$TMP/verdict-paths")" in
  "$TMPDIR"/orch-review.*/verdict) PASSES=$((PASSES + 1)) ;;
  *) FAILS=$((FAILS + 1)); echo "FAIL verdict must live in a mktemp dir outside the worktree and .orch: $(cat "$TMP/verdict-paths")" >&2 ;;
esac

echo "# pr-fix: two changes_requested reviews block the PR (M12)"
rm -f "$TMP/reviews"
export ORCH_RUN_ID="prfix-finish-2"
set +e
out="$(STUB_REVIEW=changes_requested bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr 77 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc=$?
set -e
assert_eq "$rc" "1" "pr-fix blocked after two changes_requested"
assert_contains "$out" "blocked: PR 77 review: changes_requested" "review block reason"
assert_eq "$(wc -l <"$TMP/reviews" | tr -d ' ')" "2" "one fix iteration then one re-review"

summary
