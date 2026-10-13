#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/frontier.sh"

# --- pure parser -----------------------------------------------------------
assert_eq "$(orch_parse_blockers 'Blocked by: #12, #15')" "12 15" "comma + hash"
assert_eq "$(orch_parse_blockers 'blocked by: 7')" "7" "bare number, lowercase"
assert_eq "$(orch_parse_blockers '## Question
something
Blocked by: #3 #4
more')" "3 4" "line within a body"
assert_eq "$(orch_parse_blockers 'no dependency here')" "" "no blockers → empty"
assert_eq "$(orch_parse_blockers 'Blocked by: #5, #5')" "5" "dedup repeats"

# --- partition with a stubbed gh -------------------------------------------
# Layout: #20 blocked by closed #12 → frontier; #21 blocked by open #15 → held;
#         #22 no blockers → frontier.
_ORIG_PATH="$PATH"
FAKE="$(mktemp -d)"
STUB_BASH="$(command -v bash)"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE"' EXIT
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
# args: issue view <num> --repo R --json body|state -q .<field>
num="\$3"
case "\$*" in
  *"--json state"*)
    case "\$num" in 12) echo CLOSED ;; *) echo OPEN ;; esac ;;
  *"--json body"*)
    case "\$num" in
      20) echo "Goal: x
Blocked by: #12" ;;
      21) echo "Goal: y
Blocked by: #15" ;;
      *) echo "Goal: z" ;;
    esac ;;
esac
EOF
chmod +x "$FAKE/gh"
PATH="$FAKE:$_ORIG_PATH"

orch_frontier_partition "owner/repo" 20 21 22
assert_eq "${FRONTIER_IDS[*]}" "20 22" "frontier = unblocked + closed-blocker"
assert_eq "${HELD_IDS[*]}" "21" "held = open blocker"
assert_eq "${HELD_REASON[*]}" "15" "held reason = the open blocker"

summary
