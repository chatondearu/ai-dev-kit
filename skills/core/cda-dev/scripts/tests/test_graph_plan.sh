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

summary
