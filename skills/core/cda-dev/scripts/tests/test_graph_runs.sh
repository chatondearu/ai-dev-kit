#!/usr/bin/env bash
# Per-run live graphs (#178): one file per run, current.json pointer, session-scoped
# graph-node.sh targeting, finish archives, stale runs archived. Fake tmux on PATH only.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"
GN="$TROOT/../../../cda-graph/scripts/graph-node.sh"

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin" "$TMP/repo"
# Fake tmux: `display-message -p '#S'` prints $FAKE_SESSION.
printf '#!/bin/sh\nprintf "%%s\\n" "$FAKE_SESSION"\n' >"$TMP/bin/tmux"
chmod +x "$TMP/bin/tmux"
export PATH="$TMP/bin:$PATH" ORCH_REPO="$TMP/repo"
unset ORCH_RUN_ID TMUX
G="$TMP/repo/.orch/graph"

in_session() { local s="$1"; shift; TMUX=fake FAKE_SESSION="$s" bash "$GN" "$@" --repo "$TMP/repo"; }

# Two sessions: two run files, each carrying its session and a pid.
fa="$(in_session sa init)"
fb="$(in_session sb init)"
assert_eq "$(dirname "$fa")" "$G/live" "init writes under live/"
[ "$fa" != "$fb" ] && assert_eq ok ok "two sessions, two files" || assert_eq "$fa" "different" "two sessions, two files"
assert_eq "$(jq -r .session "$fa")/$(jq -r .session "$fb")" "sa/sb" "session recorded per run"
assert_eq "$(jq -r '.pid | type' "$fa")" "number" "pid recorded"
assert_eq "$(readlink "$G/current.json")" "live/$(basename "$fb")" "current.json points at the latest run"
assert_eq "$(jq -r .run_id "$G/current.json")" "$(basename "$fb" .json)" "legacy reader parses current.json"

# Parallel writers from both sessions: no cross-writes.
( for i in 1 2 3 4 5; do in_session sa node --id "a$i" --kind worker --phase p --status running; done ) &
( for i in 1 2 3 4 5; do in_session sb node --id "b$i" --kind worker --phase p --status running; done ) &
wait
assert_eq "$(jq -c '[.nodes[].id]' "$fa")" '["a1","a2","a3","a4","a5"]' "session A nodes only in A"
assert_eq "$(jq -c '[.nodes[].id]' "$fb")" '["b1","b2","b3","b4","b5"]' "session B nodes only in B"
assert_eq "$(find "$G" -name '*.tmp.*' | wc -l | tr -d ' ')" "0" "no temp files left"

# An orch run (lib, ORCH_RUN_ID) coexists; $ORCH_RUN_ID targets it from graph-node.
( orch_graph_init orch-1; orch_graph_upsert_node integrator integrator pending queued "x" "" )
ORCH_RUN_ID=orch-1 in_session sa node --id w1 --kind worker --phase p --status running
assert_eq "$(jq -c '[.nodes[].id]' "$G/live/orch-1.json")" '["integrator","w1"]' "ORCH_RUN_ID targets the orch run"
assert_eq "$(jq '.nodes | length' "$fa")" "5" "orch run left session A alone"
in_session sb node --id b6 --kind worker --phase p --status running --run-id "$(basename "$fa" .json)"
assert_eq "$(jq -r '.nodes[-1].id' "$fa")" "b6" "--run-id overrides the session"

# No run id and no session: clear failure, nothing written.
rc=0; err="$(bash "$GN" node --id z --kind k --phase p --status s --repo "$TMP/repo" 2>&1)" || rc=$?
assert_eq "$rc" "1" "no run resolvable fails"
assert_contains "$err" "--run-id" "failure says how to target a run"
rc=0; in_session nosuch node --id z --kind k --phase p --status s 2>/dev/null || rc=$?
assert_eq "$rc" "1" "session without a run fails"

# finish archives; current.json stays on the latest run (orch-1), then follows it to runs/.
in_session sa finish --outcome ok
assert_file "$G/runs/$(basename "$fa")" "finish archives A"
[ -e "$fa" ] && assert_eq present absent "live A removed" || assert_eq ok ok "live A removed"
assert_eq "$(jq -r .status "$G/runs/$(basename "$fa")")" "ok" "archived status ok"
assert_eq "$(readlink "$G/current.json")" "live/orch-1.json" "current.json still on the latest run"
( export ORCH_RUN_ID=orch-1; orch_graph_finish blocked )
assert_eq "$(readlink "$G/current.json")" "runs/orch-1.json" "current.json follows the finished latest run"
assert_eq "$(jq -r .status "$G/current.json")" "blocked" "legacy reader sees the finished run"
in_session sb finish
assert_file "$G/runs/$(basename "$fb")" "finish archives B"

# Stale: dead pid and >24 h old is archived on the next init; live pid or fresh file stays.
sleep 0 & dead=$!; wait "$dead"
printf '{"run_id":"old","pid":%s}\n' "$dead" >"$G/live/old.json"
printf '{"run_id":"alive","pid":%s}\n' "$$" >"$G/live/alive.json"
printf '{"run_id":"fresh","pid":%s}\n' "$dead" >"$G/live/fresh.json"
touch -t 202001010000 "$G/live/old.json" "$G/live/alive.json"
in_session sc init >/dev/null
assert_file "$G/runs/old.json" "stale run archived"
assert_file "$G/live/alive.json" "run with live pid kept"
assert_file "$G/live/fresh.json" "recent run kept"

summary
