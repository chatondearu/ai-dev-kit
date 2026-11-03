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

summary
