#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"

PLAN="$(mktemp)"
cat >"$PLAN" <<'EOF'
{
  "issue": 7,
  "tasks": [
    { "id": "a", "paths": ["src/a/"], "prompt": "do a" },
    { "id": "b", "paths": ["src/b/"], "prompt": "do b" }
  ]
}
EOF

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  rm -f "$PLAN"
  exit 0
fi

n="$(jq '.tasks | length' "$PLAN")"
assert_eq "$n" "2" "plan parses two tasks"
issue="$(jq -r '.issue' "$PLAN")"
assert_eq "$issue" "7" "plan issue id"

bash "$TROOT/../orch.sh" --mode ready-pickup --issue 7 --graph-plan "$PLAN" --dry-run >/tmp/gp-dry.txt
assert_contains "$(cat /tmp/gp-dry.txt)" "graph-plan" "dry-run mentions graph-plan"
rm -f /tmp/gp-dry.txt "$PLAN"

# Task `tier`: the validator accepts small|medium|large and rejects anything else.
VAL="$TROOT/../../../cda-graph/scripts/graph-plan-validate.sh"
PLAN="$(mktemp)"
printf '{"issue":7,"tasks":[{"id":"a","paths":["src/a/"],"prompt":"x","tier":"large"}]}\n' >"$PLAN"
out="$(bash "$VAL" "$PLAN" 2>&1)"
assert_contains "$out" "a: src/a/ (tier large)" "validator accepts a task tier"
printf '{"issue":7,"tasks":[{"id":"a","paths":["src/a/"],"prompt":"x","tier":"huge"}]}\n' >"$PLAN"
rc=0; out="$(bash "$VAL" "$PLAN" 2>&1)" || rc=$?
assert_eq "$rc" "1" "validator rejects an unknown tier"
assert_contains "$out" "tier must be small, medium or large (task a: huge)" "unknown tier message"
rm -f "$PLAN"

summary
