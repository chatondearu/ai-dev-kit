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

cat >"$STUB_BIN/stub-agent" <<EOF
#!/usr/bin/env bash
cat >/dev/null
touch "$TMP/agent-ran"
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

summary
