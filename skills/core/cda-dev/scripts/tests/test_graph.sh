#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"

TMP="$(mktemp -d)"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

export ORCH_REPO="$TMP/repo"
export ORCH_GRAPH=1
export CDAMUX_HOME="$TMP/cdamux"
export ORCH_TMUX=0
mkdir -p "$ORCH_REPO" "$CDAMUX_HOME"
orch_test_stub_cdamux "$TMP"

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

orch_graph_init "test-run"
gf="$(orch_graph_file)"
assert_file "$gf" "graph file created under CDAMUX_HOME"
status="$(jq -r '.status' "$gf")"
assert_eq "$status" "running" "init sets status running"
assert_eq "$(test -e "$ORCH_REPO/.orch/graph/current.json" && echo yes || echo no)" "no" \
  "no .orch/graph writer"

orch_graph_upsert_node "worker-1" worker agent running "hello" ""
orch_graph_add_edge integrator worker-1 aggregates

nodes="$(jq '.nodes | length' "$gf")"
assert_eq "$nodes" "1" "one node after upsert"

edges="$(jq '.edges | length' "$gf")"
assert_eq "$edges" "1" "one edge after add"

orch_graph_upsert_node "worker-1" worker done done "finished" ""
phase="$(jq -r '.nodes[] | select(.id=="worker-1") | .status' "$gf")"
assert_eq "$phase" "done" "upsert updates status"

ORCH_AGENT_CLI=claude ORCH_AGENT_MODEL=sonnet ORCH_AGENT_EFFORT= ORCH_AGENT_TIER=medium \
  orch_graph_upsert_node "worker-1-implement-1" agent implement done "agent exit 0" worker-1 agent
assert_eq "$(jq -c '.nodes[] | select(.id=="worker-1-implement-1") | [.cli, .model, .effort, .tier]' "$gf")" \
  '["claude","sonnet",null,"medium"]' "agent node carries cli/model/effort/tier"
assert_eq "$(jq -r '.nodes[] | select(.id=="worker-1") | has("tier")' "$gf")" "false" "no agent arg → no tier field"

export -f orch_graph_enabled orch_graph_writable orch_graph_file orch_graph_now orch_graph_init orch_graph_session
export -f orch_graph_point_current orch_graph_archive orch_graph_archive_stale
export -f orch_graph_locked_jq orch_graph_upsert_node orch_graph_add_edge orch_graph_finish
export -f orch_cdamux_available orch_cdamux_state_set orch_cdamux_state_get orch_cdamux_home
export -f orch_cdamux_publish_run orch_cdamux_publish_run_json orch_cdamux_publish_worker
export -f orch_cdamux_mint_agent_id orch_cdamux_worker_agent_id orch_cdamux_status_from_phase
export -f orch_cdamux_run_path orch_cdamux_run_lock orch_cdamux_agent_name
bash -c 'orch_graph_upsert_node "worker-sub" worker agent running "from-subshell" ""'
sub="$(jq -r '.nodes[] | select(.id=="worker-sub") | .message' "$gf")"
assert_eq "$sub" "from-subshell" "upsert via bash -c writes node"

orch_graph_finish ok
fin_status="$(jq -r '.status' "$gf")"
assert_eq "$fin_status" "ok" "finish sets status ok"
ended="$(jq -r '.ended_at // empty' "$gf")"
if [ -n "$ended" ]; then
  assert_eq "has-ended" "has-ended" "finish sets ended_at"
else
  assert_eq "missing-ended" "has-ended" "finish sets ended_at"
fi
assert_eq "$(test -d "$ORCH_REPO/.orch/graph" && echo yes || echo no)" "no" "no .orch/graph dir"

# RMW: failed `state get` with an on-disk run must read the file, not invent a skeleton.
orch_graph_init "rmw-keep"
gf="$(orch_graph_file)"
orch_graph_upsert_node "keep-me" worker agent running "original" ""
orch_cdamux_state_get() { return 1; }
export -f orch_cdamux_state_get
orch_graph_upsert_node "keep-me" worker agent running "updated" ""
assert_eq "$(jq -r '.nodes[] | select(.id=="keep-me") | .message' "$gf")" "updated" \
  "RMW updates from on-disk run when get fails"
assert_eq "$(jq '.nodes | length' "$gf")" "1" "RMW does not wipe nodes when get fails"

summary
