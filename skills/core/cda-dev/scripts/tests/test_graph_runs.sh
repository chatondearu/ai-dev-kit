#!/usr/bin/env bash
# Per-run graphs under $CDAMUX_HOME/runs (#252): session-scoped graph-node.sh,
# finish marks status (no archive move). Fake tmux on PATH only.
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
export CDAMUX_HOME="$TMP/cdamux" ORCH_TMUX=0
mkdir -p "$CDAMUX_HOME"
orch_test_stub_cdamux "$TMP"
# Fake tmux: `display-message -p '#S'` prints $FAKE_SESSION.
printf '#!/bin/sh\nprintf "%%s\\n" "$FAKE_SESSION"\n' >"$TMP/bin/tmux"
chmod +x "$TMP/bin/tmux"
export PATH="$TMP/bin:$PATH" ORCH_REPO="$TMP/repo"
unset ORCH_RUN_ID TMUX
RUNS="$CDAMUX_HOME/runs"

in_session() { local s="$1"; shift; TMUX=fake FAKE_SESSION="$s" bash "$GN" "$@" --repo "$TMP/repo"; }

# Two sessions: two run files, each carrying its session and a pid.
fa="$(in_session sa init)"
fb="$(in_session sb init)"
assert_eq "$(dirname "$fa")" "$RUNS" "init writes under CDAMUX_HOME/runs/"
[ "$fa" != "$fb" ] && assert_eq ok ok "two sessions, two files" || assert_eq "$fa" "different" "two sessions, two files"
assert_eq "$(jq -r .session "$fa")/$(jq -r .session "$fb")" "sa/sb" "session recorded per run"
assert_eq "$(jq -r '.pid | type' "$fa")" "number" "pid recorded"
assert_eq "$(jq -r .status "$fa")" "running" "init status running"

# Parallel writers from both sessions: no cross-writes.
( for i in 1 2 3 4 5; do in_session sa node --id "a$i" --kind worker --phase p --status running; done ) &
( for i in 1 2 3 4 5; do in_session sb node --id "b$i" --kind worker --phase p --status running; done ) &
wait
assert_eq "$(jq -c '[.nodes[].id]' "$fa")" '["a1","a2","a3","a4","a5"]' "session A nodes only in A"
assert_eq "$(jq -c '[.nodes[].id]' "$fb")" '["b1","b2","b3","b4","b5"]' "session B nodes only in B"

# An orch run (lib, ORCH_RUN_ID) coexists; $ORCH_RUN_ID targets it from graph-node.
( orch_graph_init orch-1; orch_graph_upsert_node integrator integrator pending queued "x" "" )
ORCH_RUN_ID=orch-1 in_session sa node --id w1 --kind worker --phase p --status running
assert_eq "$(jq -c '[.nodes[].id]' "$RUNS/orch-1.json")" '["integrator","w1"]' "ORCH_RUN_ID targets the orch run"
assert_eq "$(jq '.nodes | length' "$fa")" "5" "orch run left session A alone"
in_session sb node --id b6 --kind worker --phase p --status running --run-id "$(basename "$fa" .json)"
assert_eq "$(jq -r '.nodes[-1].id' "$fa")" "b6" "--run-id overrides the session"

# No run id and no session: clear failure, nothing written.
rc=0; err="$(bash "$GN" node --id z --kind k --phase p --status s --repo "$TMP/repo" 2>&1)" || rc=$?
assert_eq "$rc" "1" "no run resolvable fails"
assert_contains "$err" "--run-id" "failure says how to target a run"
rc=0; in_session nosuch node --id z --kind k --phase p --status s 2>/dev/null || rc=$?
assert_eq "$rc" "1" "session without a run fails"

# finish marks status; file stays under runs/ (no archive move).
in_session sa finish --outcome ok
assert_file "$fa" "finish keeps A under runs/"
assert_eq "$(jq -r .status "$fa")" "ok" "finished status ok"
assert_eq "$(jq -r '.ended_at | type' "$fa")" "string" "finish sets ended_at"
( export ORCH_RUN_ID=orch-1; orch_graph_finish blocked )
assert_eq "$(jq -r .status "$RUNS/orch-1.json")" "blocked" "orch finish sets blocked"
in_session sb finish
assert_eq "$(jq -r .status "$fb")" "ok" "finish B ok"

# Newest session run for resolve skips finished runs (status != running).
fc="$(in_session sa init)"
[ "$fc" != "$fa" ] && assert_eq ok ok "new init after finish" || assert_eq "$fc" "different" "new init after finish"
assert_eq "$(jq -r .status "$fc")" "running" "new run is running"
in_session sa node --id anew --kind worker --phase p --status running
assert_eq "$(jq -r '.nodes[-1].id' "$fc")" "anew" "session resolve picks newest running run"

summary
