#!/usr/bin/env bash
# Gate: orch writers must not recreate status.json / .orch/graph live paths (#252).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
LIB="$TROOT/../lib"
GN="$(cd "$TROOT/../../../cda-graph/scripts" && pwd)/graph-node.sh"

hits="$(
  rg -n -e 'status\.json' -e '\.orch/graph/(live|runs|current)' \
    "$LIB"/*.sh "$GN" 2>/dev/null \
    | grep -Ev 'removed status\.json|no status\.json|hard cut|#252|not the removed' \
    || true
)"

if [ -n "$hits" ]; then
  printf 'FAIL forbidden writer path in orch libs:\n%s\n' "$hits" >&2
  FAILS=$((FAILS + 1))
else
  assert_eq ok ok "orch libs have no status.json / .orch/graph writers"
fi

summary
