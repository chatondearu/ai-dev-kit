#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/frontier.sh"

# --- pure parser -----------------------------------------------------------
if orch_is_issue_number 12; then rc=0; else rc=1; fi
assert_eq "$rc" "0" "numeric id is a GitHub issue"
if orch_is_issue_number foam-only; then rc=0; else rc=1; fi
assert_eq "$rc" "1" "slug is not a GitHub issue"

assert_eq "$(orch_parse_blockers 'Blocked by: #12, #15')" "12 15" "comma + hash"
assert_eq "$(orch_parse_blockers 'blocked by: #7')" "7" "lowercase"
assert_eq "$(orch_parse_blockers 'Blocked by: #12 (ADR-004 must land; see PR 3)')" "12" "trailing prose numbers ignored"
assert_eq "$(orch_parse_blockers '## Question
something
Blocked by: #3 #4
more')" "3 4" "line within a body"
assert_eq "$(orch_parse_blockers 'no dependency here')" "" "no blockers → empty"
assert_eq "$(orch_parse_blockers 'Blocked by: #5, #5')" "5" "dedup repeats"

assert_eq "$(orch_gh_repo_slug 'owner/repo')" "owner/repo" "slug passthrough"
if orch_gh_repo_slug '/tmp/not-a-repo-orch-frontier-test' 2>/dev/null; then
  echo "FAIL: orch_gh_repo_slug should not resolve nonsense path" >&2
  exit 1
fi

# Inherited GH_REPO must not override the repo at the given path.
_SLUG_BIN="$(mktemp -d)"
printf '#!%s\necho "${GH_REPO:-from-cwd}"\n' "$(command -v bash)" >"$_SLUG_BIN/gh"
chmod +x "$_SLUG_BIN/gh"
assert_eq "$(PATH="$_SLUG_BIN:$PATH" GH_REPO=other/repo orch_gh_repo_slug "$_SLUG_BIN")" "from-cwd" "slug ignores inherited GH_REPO"
rm -rf "$_SLUG_BIN"

# --- partition with a stubbed gh -------------------------------------------
# Layout: #20 blocked by closed #12 → frontier; #21 blocked by open #15 → held;
#         #22 no blockers → frontier; #23 body fetch fails → held.
_ORIG_PATH="$PATH"
FAKE="$(mktemp -d)"
STUB_BASH="$(command -v bash)"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE"' EXIT
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
num="\$3"
case "\$*" in
  *"--json state"*)
    case "\$num" in 12) echo CLOSED ;; *) echo OPEN ;; esac ;;
  *"--json body"*)
    case "\$num" in
      20) printf '%s\n' '{"body":"Goal: x\nBlocked by: #12"}' ;;
      21) printf '%s\n' '{"body":"Goal: y\nBlocked by: #15"}' ;;
      23) exit 1 ;;
      *) printf '%s\n' '{"body":"Goal: z"}' ;;
    esac ;;
esac
EOF
chmod +x "$FAKE/gh"
PATH="$FAKE:$_ORIG_PATH"

orch_frontier_partition "owner/repo" 20 21 22
assert_eq "${FRONTIER_IDS[*]}" "20 22" "frontier = unblocked + closed-blocker"
assert_eq "${HELD_IDS[*]}" "21" "held = open blocker"
assert_eq "${HELD_REASON[*]}" "15" "held reason = the open blocker"

orch_frontier_partition "owner/repo" 23
assert_eq "${#FRONTIER_IDS[@]}" "0" "body fetch fail → not frontier"
assert_eq "${HELD_IDS[*]}" "23" "body fetch fail → held"
assert_eq "${HELD_REASON[*]}" "body-fetch-failed" "held reason = body-fetch-failed"

summary
