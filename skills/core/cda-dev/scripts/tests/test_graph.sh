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

orch_graph_upsert_node "worker-1" worker agent running "hello" ""
orch_graph_add_edge integrator worker-1 aggregates

nodes="$(jq '.nodes | length' "$TMP/.orch/graph/current.json")"
assert_eq "$nodes" "1" "one node after upsert"

edges="$(jq '.edges | length' "$TMP/.orch/graph/current.json")"
assert_eq "$edges" "1" "one edge after add"

orch_graph_upsert_node "worker-1" worker done done "finished" ""
phase="$(jq -r '.nodes[] | select(.id=="worker-1") | .status' "$TMP/.orch/graph/current.json")"
assert_eq "$phase" "done" "upsert updates status"

summary
