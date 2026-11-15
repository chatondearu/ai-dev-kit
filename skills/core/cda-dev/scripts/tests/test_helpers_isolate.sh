#!/usr/bin/env bash
# #361: helpers redirect CDAMUX_HOME and drop agent id so publish never hits live state.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"

SENTINEL="$(mktemp -d)"
export CDAMUX_HOME="$SENTINEL" CDAMUX_AGENT_ID=a-live-orch
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"

assert_eq "${CDAMUX_AGENT_ID-unset}" "unset" "helpers unsets CDAMUX_AGENT_ID"
assert_eq "$([ "$CDAMUX_HOME" != "$SENTINEL" ] && echo ok || echo bad)" "ok" \
  "helpers replaces CDAMUX_HOME with a temp dir"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP" "$SENTINEL"' EXIT
orch_test_stub_cdamux "$TMP"
orch_graph_init "isolate-361"
assert_file "$CDAMUX_HOME/runs/isolate-361.json" "publish goes to helpers temp home"
assert_eq "$(find "$SENTINEL" -mindepth 1 | wc -l | tr -d ' ')" "0" \
  "sentinel (fake live CDAMUX_HOME) stays empty"

summary
