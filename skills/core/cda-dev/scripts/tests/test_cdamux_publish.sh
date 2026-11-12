#!/usr/bin/env bash
# Publish path: orch graph/progress → cdamux state under temp CDAMUX_HOME (#252 hard cut).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"
source "$TROOT/../lib/progress.sh"

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
HOME_DIR="$TMP/cdamux-home"
REPO="$TMP/repo"
mkdir -p "$HOME_DIR" "$REPO"
export ORCH_REPO="$REPO" ORCH_PROGRESS=0 CDAMUX_HOME="$HOME_DIR" ORCH_TMUX=0
unset ORCH_RUN_ID CDAMUX_AGENT_ID
orch_test_stub_cdamux "$TMP"

# --- (1) publish with cdamux: run + nodes + worker Agent ---
orch_graph_init "run-pub-1"
orch_graph_upsert_node "worker-42" worker agent running "hello" ""
ORCH_PROGRESS=0 orch_progress_set 42 agent "iter 1" 2>/dev/null

assert_file "$HOME_DIR/runs/run-pub-1.json" "run published under CDAMUX_HOME"
assert_eq "$(jq -r .id "$HOME_DIR/runs/run-pub-1.json")" "run-pub-1" "run id field"
assert_eq "$(jq -r .parent "$HOME_DIR/runs/run-pub-1.json")" "null" "no parent without CDAMUX_AGENT_ID"
assert_eq "$(jq -r '.nodes[] | select(.id=="worker-42") | .status' "$HOME_DIR/runs/run-pub-1.json")" \
  "running" "nodes mirrored into run"
assert_eq "$(test -e "$REPO/.orch/graph/live/run-pub-1.json" && echo yes || echo no)" "no" \
  "no repo graph writer"
assert_eq "$(test -e "$REPO/.orch/run/status.json" && echo yes || echo no)" "no" \
  "no status.json aggregate"

aid="$(cat "$REPO/.orch/run/42.agent_id")"
assert_file "$HOME_DIR/agents/${aid}.json" "worker Agent published"
assert_eq "$(jq -r .role "$HOME_DIR/agents/${aid}.json")" "worker" "agent role"
assert_eq "$(jq -r .run "$HOME_DIR/agents/${aid}.json")" "run-pub-1" "agent linked to run"
assert_eq "$(jq -r .parent "$HOME_DIR/agents/${aid}.json")" "null" "worker parent null without orch"
assert_eq "$(jq -r .status "$HOME_DIR/agents/${aid}.json")" "working" "phase agent → working"
assert_eq "$(jq -r .phase "$HOME_DIR/agents/${aid}.json")" "agent" "phase field on agent"

# --- (2) sub-run: CDAMUX_AGENT_ID becomes run.parent ---
export CDAMUX_AGENT_ID="a-parent1"
orch_graph_init "run-sub-1"
orch_graph_upsert_node "w1" worker pending queued "q" ""
ORCH_PROGRESS=0 orch_progress_set 7 pending "queued" 2>/dev/null
assert_eq "$(jq -r .parent "$HOME_DIR/runs/run-sub-1.json")" "a-parent1" "run is sub-run of CDAMUX_AGENT_ID"
aid7="$(cat "$REPO/.orch/run/7.agent_id")"
assert_eq "$(jq -r .parent "$HOME_DIR/agents/${aid7}.json")" "a-parent1" "worker parent is orchestrator"
assert_eq "$(jq -r .status "$HOME_DIR/agents/${aid7}.json")" "idle" "phase pending → idle"
unset CDAMUX_AGENT_ID

# --- (3) graph-node.sh surface still works and publishes ---
GN="$TROOT/../../../cda-graph/scripts/graph-node.sh"
unset ORCH_RUN_ID
gf="$(bash "$GN" init --repo "$REPO" --run-id "gn-1")"
assert_file "$gf" "graph-node init writes CDAMUX_HOME run"
assert_file "$HOME_DIR/runs/gn-1.json" "graph-node init publishes run"
bash "$GN" node --repo "$REPO" --run-id gn-1 --id n1 --kind worker --phase p --status running
assert_eq "$(jq -r '.nodes[0].id' "$HOME_DIR/runs/gn-1.json")" "n1" "graph-node node publishes"
bash "$GN" finish --repo "$REPO" --run-id gn-1 --outcome ok
assert_eq "$(jq -r .status "$HOME_DIR/runs/gn-1.json")" "ok" "graph-node finish publishes status"

# --- (4) parallel upsert: published run must keep all nodes (flock on RMW) ---
if command -v flock >/dev/null 2>&1; then
  export -f orch_graph_enabled orch_graph_writable orch_graph_file orch_graph_now orch_graph_init orch_graph_session
  export -f orch_graph_point_current orch_graph_archive orch_graph_archive_stale
  export -f orch_graph_locked_jq orch_graph_upsert_node orch_graph_add_edge orch_graph_finish
  export -f orch_cdamux_available orch_cdamux_state_set orch_cdamux_state_get orch_cdamux_home
  export -f orch_cdamux_publish_run orch_cdamux_publish_run_json orch_cdamux_publish_worker
  export -f orch_cdamux_mint_agent_id orch_cdamux_worker_agent_id orch_cdamux_status_from_phase
  export -f orch_cdamux_run_path orch_cdamux_run_lock orch_cdamux_agent_name
  export -f orch_cdamux_msg_send orch_cdamux_parent_msg_worker orch_cdamux_parent_msg_run_end
  unset ORCH_RUN_ID
  orch_graph_init "run-race-1"
  export CDAMUX_STUB_SLEEP_RUNS=invert
  for i in 1 2 3 4 5 6 7 8; do
    (
      orch_graph_upsert_node "w-race-$i" worker agent running "r$i" ""
    ) &
  done
  wait
  unset CDAMUX_STUB_SLEEP_RUNS
  pub_ids="$(jq -r '[.nodes[].id] | sort | join(",")' "$HOME_DIR/runs/run-race-1.json")"
  assert_eq "$(jq '.nodes | length' "$HOME_DIR/runs/run-race-1.json")" "8" "all 8 race nodes published"
  assert_contains "$pub_ids" "w-race-1" "race node 1 present"
else
  echo "SKIP parallel publish race (flock not installed)"
fi

# --- (5) no-op without cdamux on PATH: pipeline continues, no CDAMUX_HOME writes ---
_clean=""
_old_ifs="$IFS"
IFS=:
for _d in $PATH; do
  [ -n "$_d" ] || continue
  [ -x "$_d/cdamux" ] && continue
  _clean="${_clean:+$_clean:}$_d"
done
IFS="$_old_ifs"
export PATH="$_clean"
unset _clean _old_ifs _d
! command -v cdamux >/dev/null 2>&1
orch_cdamux_available && assert_eq yes no "cdamux must be absent" || assert_eq ok ok "cdamux absent"
rm -rf "$HOME_DIR/runs" "$HOME_DIR/agents"
mkdir -p "$HOME_DIR"
orch_graph_init "run-noop"
orch_graph_upsert_node "x" worker agent running "y" ""
ORCH_PROGRESS=0 orch_progress_set 99 agent "no cdamux" 2>/dev/null
assert_file "$REPO/.orch/run/99.json" "worker artefact still written without cdamux"
assert_eq "$(test -e "$REPO/.orch/graph/live/run-noop.json" && echo yes || echo no)" "no" \
  "no graph without cdamux"
if [ -e "$HOME_DIR/runs/run-noop.json" ]; then
  assert_eq present absent "no CDAMUX_HOME run without cdamux"
else
  assert_eq ok ok "no CDAMUX_HOME run without cdamux"
fi

summary
