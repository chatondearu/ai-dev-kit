#!/usr/bin/env bash
# Stub tests for closest_status_option (exact match only).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=../gh-board.sh
# Extract only the function by sourcing is heavy (needs gh); redefine locally
# to match gh-board.sh contract.
closest_status_option() {
  local want="$1" options_json="$2"
  printf '%s' "$options_json" | jq -r --arg w "$want" '
    ($w | ascii_downcase) as $needle
    | (map(select((.name | ascii_downcase) == $needle)) | .[0]) as $exact
    | if $exact then "\($exact.id)\t\($exact.name)" else empty end
  '
}

FAILS=0
assert_eq() {
  local got="$1" want="$2" msg="$3"
  if [ "$got" = "$want" ]; then
    printf 'ok   %s\n' "$msg"
  else
    printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$msg" "$got" "$want" >&2
    FAILS=$((FAILS + 1))
  fi
}

OPTS='[{"id":"1","name":"Already Reviewed"},{"id":"2","name":"Ready"},{"id":"3","name":"Abandoned"},{"id":"4","name":"Done"}]'

got="$(closest_status_option Ready "$OPTS")"
assert_eq "$got" $'2\tReady' "exact Ready"

got="$(closest_status_option ready "$OPTS")"
assert_eq "$got" $'2\tReady' "case-insensitive Ready"

got="$(closest_status_option Done "$OPTS")"
assert_eq "$got" $'4\tDone' "exact Done"

# No "Ready" column — must NOT soft-match Already Reviewed
OPTS_NO_READY='[{"id":"1","name":"Already Reviewed"},{"id":"3","name":"Abandoned"}]'
got="$(closest_status_option Ready "$OPTS_NO_READY")"
assert_eq "$got" "" "Ready does not match Already Reviewed"

# No "Done" — must NOT soft-match Abandoned
OPTS_NO_DONE='[{"id":"1","name":"Already Reviewed"},{"id":"3","name":"Abandoned"}]'
got="$(closest_status_option Done "$OPTS_NO_DONE")"
assert_eq "$got" "" "Done does not match Abandoned"

# Drift guard: live function in gh-board.sh must still be exact-only (no contains)
if grep -q 'contains(\$needle)' "$ROOT/gh-board.sh"; then
  printf 'FAIL gh-board.sh still uses soft contains match\n' >&2
  FAILS=$((FAILS + 1))
else
  printf 'ok   gh-board.sh has no soft contains\n'
fi

if [ "$FAILS" -ne 0 ]; then
  printf '%s failed\n' "$FAILS" >&2
  exit 1
fi
printf 'all ok\n'
