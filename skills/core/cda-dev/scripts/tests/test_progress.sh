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
export ORCH_TMUX=0
export CDAMUX_HOME="$TMP/cdamux"
mkdir -p "$CDAMUX_HOME"
orch_test_stub_cdamux "$TMP"

orch_progress_set 42 agent "iter 1/10"
assert_file "$TMP/.orch/run/42.status" "status file written"
line="$(cat "$TMP/.orch/run/42.status")"
assert_eq "$line" "agent|iter 1/10" "status format phase|msg"
assert_eq "$(test -e "$TMP/.orch/run/status.json" && echo yes || echo no)" "no" "no status.json aggregate"

icon="$(orch_progress_icon done x)"
assert_eq "$icon" "✓" "done icon"
icon="$(orch_progress_icon blocked x)"
assert_eq "$icon" "✗" "blocked icon"
icon="$(orch_progress_icon agent ⠋)"
assert_eq "$icon" "⠋" "spinner frame for active phase"

export ORCH_PROGRESS=1
orch_progress_enabled
assert_eq "$?" "0" "ORCH_PROGRESS=1 enables"
export ORCH_PROGRESS=0
set +e
orch_progress_enabled
rc=$?
set -e
assert_eq "$rc" "1" "ORCH_PROGRESS=0 disables"

# Parallel writers: per-worker <id>.json replaced atomically; no status.json.
if command -v jq >/dev/null 2>&1; then
  for w in 1 2 3 4; do
    ( for i in $(seq 1 25); do orch_progress_set "w$w" agent "iter $i" 2>/dev/null; done ) &
  done
  bad=0
  while [ -n "$(jobs -rp)" ]; do
    for f in "$TMP/.orch/run"/w*.json "$TMP/.orch/run/42.json"; do
      [ -f "$f" ] || continue
      jq -e . "$f" >/dev/null 2>&1 || bad=$((bad + 1))
    done
  done
  wait
  assert_eq "$bad" "0" "worker json never read half-written during parallel writes"
  assert_eq "$(ls "$TMP/.orch/run"/w*.json "$TMP/.orch/run/42.json" 2>/dev/null | wc -l | tr -d ' ')" "5" \
    "all workers have per-id json"
  assert_eq "$(find "$TMP/.orch/run" -name '*.tmp.*' | wc -l | tr -d ' ')" "0" "no temp files left behind"
fi

# Phase history on per-worker json + CDAMUX agent.
if command -v jq >/dev/null 2>&1; then
  rm -rf "$TMP/.orch" "$CDAMUX_HOME"
  mkdir -p "$TMP/.orch/run" "$CDAMUX_HOME"
  printf '{"id":"7","phase":"agent","message":"old","updated_at":"t"}\n' >"$TMP/.orch/run/7.json"
  orch_progress_set 7 agent "legacy file without phases" 2>/dev/null
  orch_progress_set 7 agent "iter 2" 2>/dev/null
  orch_progress_set 7 verify "make test" 2>/dev/null
  assert_eq "$(jq -c '[.phases[].phase]' "$TMP/.orch/run/7.json")" '["agent","verify"]' "one history entry per phase change"
  assert_eq "$(jq -r '.phases[0].started_at | type' "$TMP/.orch/run/7.json")" "number" "started_at is epoch seconds"
  aid="$(cat "$TMP/.orch/run/7.agent_id")"
  assert_eq "$(jq -r .phase "$CDAMUX_HOME/agents/${aid}.json")" "verify" "agent phase published"
  orch_progress_set 7 pending "queued" 2>/dev/null
  assert_eq "$(jq -c '[.phases[].phase]' "$TMP/.orch/run/7.json")" '["pending"]' "pending resets the history"
  assert_eq "$(jq -r .status "$CDAMUX_HOME/agents/${aid}.json")" "idle" "pending → idle on agent"
  ORCH_RUN_ID=r9 orch_progress_set 7 agent "in run" 2>/dev/null
  assert_eq "$(jq -r .run "$CDAMUX_HOME/agents/${aid}.json")" "r9" "agent carries run_id"
fi

# cli/model/effort/tier: set by orch_progress_agent, kept across later phase writes.
if command -v jq >/dev/null 2>&1; then
  orch_progress_set 8 agent "starting" 2>/dev/null
  ORCH_AGENT_CLI=claude ORCH_AGENT_MODEL=sonnet ORCH_AGENT_EFFORT=high ORCH_AGENT_TIER=medium \
    orch_progress_agent 8
  orch_progress_set 8 verify "make test" 2>/dev/null
  assert_eq "$(jq -c '[.cli, .model, .effort, .tier, .phase]' "$TMP/.orch/run/8.json")" \
    '["claude","sonnet","high","medium","verify"]' "worker json keeps cli/model/effort/tier"
  ORCH_AGENT_CLI=agent ORCH_AGENT_MODEL=auto ORCH_AGENT_EFFORT= ORCH_AGENT_TIER=pinned orch_progress_agent 8
  assert_eq "$(jq -c '[.cli, .model, has("effort"), .tier, .message]' "$TMP/.orch/run/8.json")" \
    '["agent","auto",false,"pinned","make test"]' "orch_progress_agent replaces fields, drops empty, keeps message"
  orch_progress_set 8 pending "queued" 2>/dev/null
  assert_eq "$(jq -r 'has("tier")' "$TMP/.orch/run/8.json")" "false" "pending (new run) clears the agent fields"
fi

summary
