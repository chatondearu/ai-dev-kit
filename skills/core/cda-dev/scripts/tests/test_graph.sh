#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"

TMP="$(mktemp -d)"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

export ORCH_REPO="$TMP"
export ORCH_GRAPH=1

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

orch_graph_init "test-run"
assert_file "$TMP/.orch/graph/current.json" "graph file created"
status="$(jq -r '.status' "$TMP/.orch/graph/current.json")"
assert_eq "$status" "running" "init sets status running"

orch_graph_upsert_node "worker-1" worker agent running "hello" ""
orch_graph_add_edge integrator worker-1 aggregates

nodes="$(jq '.nodes | length' "$TMP/.orch/graph/current.json")"
assert_eq "$nodes" "1" "one node after upsert"

edges="$(jq '.edges | length' "$TMP/.orch/graph/current.json")"
assert_eq "$edges" "1" "one edge after add"

orch_graph_upsert_node "worker-1" worker done done "finished" ""
phase="$(jq -r '.nodes[] | select(.id=="worker-1") | .status' "$TMP/.orch/graph/current.json")"
assert_eq "$phase" "done" "upsert updates status"

# Workers run under bash -c with exported functions (orch.sh). Regression for missing export -f.
export -f orch_graph_enabled orch_graph_file orch_graph_now orch_graph_init
export -f orch_graph_locked_jq orch_graph_upsert_node orch_graph_add_edge orch_graph_touch orch_graph_finish
bash -c 'orch_graph_upsert_node "worker-sub" worker agent running "from-subshell" ""'
sub="$(jq -r '.nodes[] | select(.id=="worker-sub") | .message' "$TMP/.orch/graph/current.json")"
assert_eq "$sub" "from-subshell" "upsert via bash -c writes node"

orch_graph_finish ok
fin_status="$(jq -r '.status' "$TMP/.orch/graph/current.json")"
assert_eq "$fin_status" "ok" "finish sets status ok"
ended="$(jq -r '.ended_at // empty' "$TMP/.orch/graph/current.json")"
if [ -n "$ended" ]; then
  assert_eq "has-ended" "has-ended" "finish sets ended_at"
else
  assert_eq "missing-ended" "has-ended" "finish sets ended_at"
fi

assert_file "$TMP/.orch/graph/runs/test-run.json" "finish archives run snapshot"

summary
