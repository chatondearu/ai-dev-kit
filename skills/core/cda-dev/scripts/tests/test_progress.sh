#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/progress.sh
source "$TROOT/../lib/progress.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export ORCH_REPO="$TMP"
export ORCH_PROGRESS=0

orch_progress_set 42 agent "iter 1/10"
assert_file "$TMP/.orch/run/42.status" "status file written"
line="$(cat "$TMP/.orch/run/42.status")"
assert_eq "$line" "agent|iter 1/10" "status format phase|msg"

icon="$(orch_progress_icon done x)"
assert_eq "$icon" "✓" "done icon"
icon="$(orch_progress_icon blocked x)"
assert_eq "$icon" "✗" "blocked icon"
icon="$(orch_progress_icon agent ⠋)"
assert_eq "$icon" "⠋" "spinner frame for active phase"

# enabled only when forced on
export ORCH_PROGRESS=1
orch_progress_enabled
assert_eq "$?" "0" "ORCH_PROGRESS=1 enables"
export ORCH_PROGRESS=0
set +e
orch_progress_enabled
rc=$?
set -e
assert_eq "$rc" "1" "ORCH_PROGRESS=0 disables"

# Parallel writers: status.json and <id>.json are replaced atomically, never half-written.
if command -v jq >/dev/null 2>&1; then
  for w in 1 2 3 4; do
    ( for i in $(seq 1 25); do orch_progress_set "w$w" agent "iter $i" 2>/dev/null; done ) &
  done
  bad=0
  while [ -n "$(jobs -rp)" ]; do
    jq -e . "$TMP/.orch/run/status.json" >/dev/null 2>&1 || bad=$((bad + 1))
  done
  wait
  assert_eq "$bad" "0" "status.json never read half-written during parallel writes"
  assert_eq "$(jq '.workers | length' "$TMP/.orch/run/status.json")" "5" "all workers aggregated (42 + w1..w4)"
  assert_eq "$(find "$TMP/.orch/run" -name '*.tmp.*' | wc -l | tr -d ' ')" "0" "no temp files left behind"
fi

# Phase history: one entry per phase change (repeated phase keeps its start), pending resets.
if command -v jq >/dev/null 2>&1; then
  rm -rf "$TMP/.orch"
  mkdir -p "$TMP/.orch/run"
  printf '{"id":"7","phase":"agent","message":"old","updated_at":"t"}\n' >"$TMP/.orch/run/7.json"
  orch_progress_set 7 agent "legacy file without phases" 2>/dev/null
  orch_progress_set 7 agent "iter 2" 2>/dev/null
  orch_progress_set 7 verify "make test" 2>/dev/null
  assert_eq "$(jq -c '[.phases[].phase]' "$TMP/.orch/run/7.json")" '["agent","verify"]' "one history entry per phase change"
  assert_eq "$(jq -r '.phases[0].started_at | type' "$TMP/.orch/run/7.json")" "number" "started_at is epoch seconds"
  assert_eq "$(jq '.workers[0].phases | length' "$TMP/.orch/run/status.json")" "2" "history aggregated into status.json"
  orch_progress_set 7 pending "queued" 2>/dev/null
  assert_eq "$(jq -c '[.phases[].phase]' "$TMP/.orch/run/7.json")" '["pending"]' "pending resets the history"
  # run_id: same id as the graph file (ORCH_RUN_ID); absent outside a run.
  assert_eq "$(jq -r '.run_id // "none"' "$TMP/.orch/run/status.json")" "none" "no run_id without ORCH_RUN_ID"
  ORCH_RUN_ID=r9 orch_progress_set 7 agent "in run" 2>/dev/null
  assert_eq "$(jq -r '.run_id' "$TMP/.orch/run/status.json")" "r9" "status.json carries run_id"
fi

# cli/model/effort/tier: set by orch_progress_agent, kept across later phase writes.
if command -v jq >/dev/null 2>&1; then
  orch_progress_set 8 agent "starting" 2>/dev/null
  ORCH_AGENT_CLI=claude ORCH_AGENT_MODEL=sonnet ORCH_AGENT_EFFORT=high ORCH_AGENT_TIER=medium \
    orch_progress_agent 8
  orch_progress_set 8 verify "make test" 2>/dev/null
  assert_eq "$(jq -c '.workers[] | select(.id=="8") | [.cli, .model, .effort, .tier, .phase]' "$TMP/.orch/run/status.json")" \
    '["claude","sonnet","high","medium","verify"]' "status.json keeps cli/model/effort/tier"
  ORCH_AGENT_CLI=agent ORCH_AGENT_MODEL=auto ORCH_AGENT_EFFORT= ORCH_AGENT_TIER=pinned orch_progress_agent 8
  assert_eq "$(jq -c '[.cli, .model, has("effort"), .tier, .message]' "$TMP/.orch/run/8.json")" \
    '["agent","auto",false,"pinned","make test"]' "orch_progress_agent replaces fields, drops empty, keeps message"
fi

summary
