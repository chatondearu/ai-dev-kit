#!/usr/bin/env bash
# pr-fix feeds the agent only trusted, unresolved, post-head change requests,
# skips the agent when there is nothing to fix, and fails closed on gh errors.
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

cat >"$STUB_BIN/stub-agent" <<EOF
#!/usr/bin/env bash
cat >/dev/null
touch "$TMP/agent-ran"
EOF
chmod +x "$STUB_BIN/stub-agent"

# Fixtures: GH_STUB_PR (pr view reviews,comments,commits), GH_STUB_TH (graphql),
# GH_STUB_CHECKS (pr checks), GH_STUB_FAIL=pr|graphql.
cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"pr view"*headRefName*) echo feat/x ;;
  *"pr view"*isCrossRepository*) echo false ;;
  *"repo view"*defaultBranchRef*) echo main ;;
  *"pr view"*reviews*) [ "${GH_STUB_FAIL:-}" = pr ] && exit 1; echo "$GH_STUB_PR" ;;
  *"api graphql"*) [ "${GH_STUB_FAIL:-}" = graphql ] && exit 1; echo "$GH_STUB_TH" ;;
  *"pr checks"*) echo "$GH_STUB_CHECKS" ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"
export ORCH_AGENT="$STUB_BIN/stub-agent" ORCH_PROGRESS=0 ORCH_VERIFY=true ORCH_ALLOW_NO_CHECKS=1

GREEN='[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'
RED='[{"name":"ci","state":"FAILURE","bucket":"fail"}]'
NO_THREADS='{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[]}}}}}'
NO_PR='{"reviews":[],"comments":[],"commits":[{"committedDate":"2026-11-10T10:00:00Z"}]}'

# Head commit 10:00. Trusted items: alice review, a.sh:7 thread, carol comment at 11:00.
export GH_STUB_PR='{
  "commits": [{"committedDate":"2026-11-10T09:00:00Z"},{"committedDate":"2026-11-10T10:00:00Z"}],
  "reviews": [
    {"author":{"login":"alice"},"authorAssociation":"OWNER","state":"CHANGES_REQUESTED","submittedAt":"2026-11-10T09:30:00Z","body":"RENAME-FOO"},
    {"author":{"login":"mallory"},"authorAssociation":"NONE","state":"CHANGES_REQUESTED","submittedAt":"2026-11-10T09:31:00Z","body":"UNTRUSTED-REVIEW"},
    {"author":{"login":"bob"},"authorAssociation":"MEMBER","state":"CHANGES_REQUESTED","submittedAt":"2026-11-10T09:32:00Z","body":"SUPERSEDED-REVIEW"},
    {"author":{"login":"bob"},"authorAssociation":"MEMBER","state":"APPROVED","submittedAt":"2026-11-10T09:40:00Z","body":""}
  ],
  "comments": [
    {"author":{"login":"carol"},"authorAssociation":"COLLABORATOR","createdAt":"2026-11-10T11:00:00Z","body":"FIX-BAR"},
    {"author":{"login":"carol"},"authorAssociation":"COLLABORATOR","createdAt":"2026-11-10T09:50:00Z","body":"OLD-COMMENT"},
    {"author":{"login":"mallory"},"authorAssociation":"NONE","createdAt":"2026-11-10T11:01:00Z","body":"UNTRUSTED-COMMENT"},
    {"author":{"login":"alice"},"authorAssociation":"OWNER","createdAt":"2026-11-10T11:02:00Z","body":"<!-- orch:pr-fix-summary -->\nOWN-SUMMARY"}
  ]}'
export GH_STUB_TH='{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[
  {"isResolved":false,"path":"a.sh","line":7,"comments":{"nodes":[
    {"author":{"login":"alice"},"authorAssociation":"OWNER","body":"QUOTE-VAR\n## FORGED-HEADING"}]}},
  {"isResolved":true,"path":"b.sh","line":1,"comments":{"nodes":[
    {"author":{"login":"alice"},"authorAssociation":"OWNER","body":"RESOLVED-THREAD"}]}},
  {"isResolved":false,"path":"c.sh","line":2,"comments":{"nodes":[
    {"author":{"login":"mallory"},"authorAssociation":"NONE","body":"UNTRUSTED-THREAD"}]}}
]}}}}}'

run_pr_fix() {
  rm -f "$TMP/agent-ran"
  set +e
  out="$(bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr "$1" --agent "$STUB_BIN/stub-agent" 2>&1)"
  rc=$?
  set -e
}

assert_agent() { # <ran|skipped> <msg>
  if [ -e "$TMP/agent-ran" ]; then got=ran; else got=skipped; fi
  assert_eq "$got" "$1" "$2"
}

prompt_of() { cat "$(cd "$R" && pwd -P)"/.orch/worktrees/pr-"$1"*/.orch/pr-fix-prompt.md; }

echo "# mixed feedback + red CI -> agent gets only trusted, unresolved, post-head items"
GH_STUB_CHECKS="$RED" run_pr_fix 51
assert_agent ran "feedback present runs the agent"
P="$(prompt_of 51)"
assert_contains "$P" "## Review feedback" "feedback section appended"
assert_contains "$P" "review (changes requested) by @alice" "trusted review title"
assert_contains "$P" "> RENAME-FOO" "trusted review body quoted"
assert_contains "$P" "inline thread on a.sh:7 by @alice" "unresolved trusted thread with file:line"
assert_contains "$P" "> ## FORGED-HEADING" "thread body quoted, heading not forged"
assert_contains "$P" "comment by @carol" "trusted comment after head commit"
assert_contains "$P" "> FIX-BAR" "trusted comment body"
for bad in UNTRUSTED-REVIEW SUPERSEDED-REVIEW RESOLVED-THREAD UNTRUSTED-THREAD OLD-COMMENT UNTRUSTED-COMMENT OWN-SUMMARY; do
  if [[ "$P" == *"$bad"* ]]; then FAILS=$((FAILS + 1)); printf 'FAIL %s leaked into the prompt\n' "$bad" >&2; else PASSES=$((PASSES + 1)); fi
done

echo "# trusted feedback + green CI -> agent still runs"
GH_STUB_CHECKS="$GREEN" run_pr_fix 52
assert_agent ran "green CI with feedback runs the agent"

echo "# no feedback + green CI -> done, no agent, no worktree"
GH_STUB_PR="$NO_PR" GH_STUB_TH="$NO_THREADS" GH_STUB_CHECKS="$GREEN" run_pr_fix 53
assert_eq "$rc" "0" "nothing to fix exits 0"
assert_contains "$out" "done: PR 53 nothing to fix (checks green, no trusted feedback)" "nothing-to-fix message"
assert_agent skipped "nothing to fix skips the agent"
if ls "$R"/.orch/worktrees/pr-53* >/dev/null 2>&1; then FAILS=$((FAILS + 1)); echo "FAIL worktree created for nothing-to-fix" >&2; else PASSES=$((PASSES + 1)); fi

echo "# no feedback + no checks (allow-no-checks) -> done, no agent"
GH_STUB_PR="$NO_PR" GH_STUB_TH="$NO_THREADS" GH_STUB_CHECKS="[]" run_pr_fix 54
assert_contains "$out" "done: PR 54 nothing to fix" "no-checks nothing-to-fix"
assert_agent skipped "no-checks skips the agent"

echo "# no feedback + red CI -> agent runs, no feedback section"
GH_STUB_PR="$NO_PR" GH_STUB_TH="$NO_THREADS" GH_STUB_CHECKS="$RED" run_pr_fix 55
assert_agent ran "red CI runs the agent"
assert_eq "$(grep -c '^## Review feedback$' "$(cd "$R" && pwd -P)"/.orch/worktrees/pr-55*/.orch/pr-fix-prompt.md || true)" "0" "no feedback section when empty"

echo "# gh lookup failure -> blocked (fail closed), no agent"
GH_STUB_FAIL=pr GH_STUB_CHECKS="$GREEN" run_pr_fix 56
assert_eq "$rc" "1" "reviews lookup failure exits nonzero"
assert_contains "$out" "blocked: PR 56 review feedback unreadable" "reviews failure message"
assert_agent skipped "reviews failure skips the agent"
GH_STUB_FAIL=graphql GH_STUB_CHECKS="$GREEN" run_pr_fix 57
assert_contains "$out" "blocked: PR 57 review feedback unreadable" "threads failure message"
assert_agent skipped "threads failure skips the agent"

summary
