#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"

GOOD="$(mktemp)"
BAD="$(mktemp)"
cleanup() { rm -f "$GOOD" "$BAD"; }
trap cleanup EXIT

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

cat >"$GOOD" <<'EOF'
{"issue":1,"tasks":[{"id":"a","paths":["src/api/"],"prompt":"x"},{"id":"b","paths":["src/ui/"],"prompt":"y"}]}
EOF

cat >"$BAD" <<'EOF'
{"issue":1,"tasks":[{"id":"a","paths":["src/"],"prompt":"x"},{"id":"b","paths":["src/ui/"],"prompt":"y"}]}
EOF

set +e
orch_graph_plan_validate_disjoint "$GOOD"
g=$?
orch_graph_plan_validate_disjoint "$BAD"
b=$?
set -e

assert_eq "$g" "0" "disjoint plan validates"
assert_eq "$b" "1" "overlapping plan rejected"

pw=0
orch_graph_plan_wants_parallel "$GOOD" || pw=$?
assert_eq "$pw" "0" "auto parallel when disjoint"

export ORCH_GRAPH_PLAN_PARALLEL=sequential
pw=0
orch_graph_plan_wants_parallel "$GOOD" || pw=$?
assert_eq "$pw" "1" "sequential disables parallel"

summary
