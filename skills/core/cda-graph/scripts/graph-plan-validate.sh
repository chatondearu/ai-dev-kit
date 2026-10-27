#!/usr/bin/env bash
# Validate a --graph-plan JSON file (issue, tasks, disjoint paths).
set -euo pipefail
DEV_LIB="$(cd "$(dirname "$0")/../../cda-dev/scripts/lib" && pwd)"
# shellcheck source=/dev/null
source "$DEV_LIB/graph.sh"

PLAN=""
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help)
      echo "usage: graph-plan-validate.sh PLAN.json"
      exit 0
      ;;
    *) PLAN="$1";;
  esac
  shift
done

[ -n "$PLAN" ] || { echo "need plan file" >&2; exit 2; }
[ -f "$PLAN" ] || { echo "missing: $PLAN" >&2; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "jq required" >&2; exit 1; }

export ORCH_REPO="$(pwd)"
export ORCH_GRAPH=0

n="$(jq '.tasks | length' "$PLAN")"
issue="$(jq -r '.issue // empty' "$PLAN")"
printf 'plan issue=%s tasks=%s\n' "${issue:-?}" "$n"

if orch_graph_plan_validate_disjoint "$PLAN"; then
  printf 'paths: disjoint (safe for ORCH_GRAPH_PLAN_PARALLEL=auto|parallel)\n'
else
  exit 1
fi

for i in $(seq 0 $((n - 1))); do
  tid="$(jq -r ".tasks[$i].id" "$PLAN")"
  paths="$(jq -r ".tasks[$i].paths | join(\", \")" "$PLAN")"
  printf '  - %s: %s\n' "$tid" "$paths"
done
