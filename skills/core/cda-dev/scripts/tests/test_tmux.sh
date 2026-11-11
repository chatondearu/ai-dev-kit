#!/usr/bin/env bash
# ORCH_TMUX=1 workers: agent runs in a tmux session with @cdamux_* options, exit code
# and output come back, session is removed. Throwaway tmux server only (TMUX_TMPDIR + -L).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"
source "$TROOT/../lib/progress.sh"

REAL_TMUX="$(command -v tmux || true)"
if [ -z "$REAL_TMUX" ]; then
  echo "skip: tmux not installed"
  summary
  exit 0
fi

TMP="$(mktemp -d)"
# Short socket dir: unix socket paths are capped near 104 bytes on macOS.
SOCK="$(mktemp -d /tmp/ot.XXXXXX)"
SRV="orchtest$$"
cleanup() {
  local p
  p="$(TMUX_TMPDIR="$SOCK" "$REAL_TMUX" -L "$SRV" display -p '#{socket_path}' 2>/dev/null || true)"
  case "$p" in "$SOCK"/*|/private"$SOCK"/*) TMUX_TMPDIR="$SOCK" "$REAL_TMUX" -L "$SRV" kill-server 2>/dev/null || true ;; esac
  rm -rf "$TMP" "$SOCK"
}
trap cleanup EXIT

unset TMUX TMUX_PANE
export TMUX_TMPDIR="$SOCK"
mkdir -p "$TMP/bin" "$TMP/repo" "$TMP/tmpdir"
# Every tmux call (lib and stub) goes to the throwaway server.
printf '#!/bin/sh\nexec %s -L %s "$@"\n' "$REAL_TMUX" "$SRV" >"$TMP/bin/tmux"
STUB_BASH="$(command -v bash)"
# Stub named cursor-agent: proves @cdamux_agent stores the product name, not the binary.
# Match the cursor CLI argv shape (-p --force … prompt); do not read stdin (would hang).
cat >"$TMP/bin/cursor-agent" <<EOF
#!$STUB_BASH
tmux show-options -t "\$(tmux display -p '#{session_name}'):" | grep cdamux >"$TMP/opts"
echo "stub-out marker=\$MARKER"
printf '%s\n' "\$*"
exit "\${STUB_RC:-0}"
EOF
chmod +x "$TMP/bin/tmux" "$TMP/bin/cursor-agent"
echo "prompt-body" >"$TMP/prompt"
export PATH="$TMP/bin:$PATH" TMPDIR="$TMP/tmpdir"
export ORCH_TMUX=1 ORCH_RUN_ID=r1 ORCH_WORKER_ID=42 ORCH_REPO="$TMP/repo"
export ORCH_AGENT="$TMP/bin/cursor-agent" MARKER=m1
S=orch-r1-42

# Prime the throwaway server with a clean client env. Otherwise the first
# `STUB_RC=7 orch_agent_run` starts the server and freezes STUB_RC into the
# global environment, so later panes inherit exit 7.
tmux new-session -d -s _boot true
tmux kill-session -t =_boot:

rc=0
out="$(STUB_RC=7 orch_agent_run "$TMP/repo" "$TMP/prompt")" || rc=$?
assert_eq "$rc" "7" "agent exit code propagated"
assert_contains "$out" "stub-out marker=m1" "output echoed back, caller env replayed"
assert_contains "$out" "prompt-body" "prompt passed as argv (cursor path)"
assert_contains "$(cat "$TMP/repo/.orch/run/42.log")" "stub-out marker=m1" "worker log written"
opts="$(cat "$TMP/opts")"
assert_contains "$opts" "@cdamux_role worker" "role option"
assert_contains "$opts" "@cdamux_run r1" "run option"
assert_contains "$opts" "@cdamux_agent cursor" "agent option is product name"
assert_contains "$opts" "@cdamux_state running" "running while the agent runs"
rc=0; tmux has-session -t "=$S:" 2>/dev/null || rc=$?
assert_eq "$rc" "1" "session removed after the agent exits"
assert_eq "$(find "$TMP/tmpdir" -type f | wc -l | tr -d ' ')" "0" "runner and rc files removed"

# KEEP: session survives with the final state; progress mirrors phases; kill_run cleans up.
rc=0
ORCH_TMUX_KEEP=1 orch_agent_run "$TMP/repo" "$TMP/prompt" >/dev/null || rc=$?
assert_eq "$rc" "0" "keep run exit 0"
assert_eq "$(tmux show-options -t "=$S:" -v @cdamux_state)" "done" "kept session marked done"
ORCH_PROGRESS=0 orch_progress_set 42 blocked "ci failed" 2>/dev/null
assert_eq "$(tmux show-options -t "=$S:" -v @cdamux_state)" "error" "progress blocked -> error"
if command -v jq >/dev/null 2>&1; then
  assert_eq "$(jq -r '.workers[0].tmux_session' "$TMP/repo/.orch/run/status.json")" "$S" "status.json tmux_session"
  assert_eq "$(jq -r '.workers[0].log' "$TMP/repo/.orch/run/status.json")" "$TMP/repo/.orch/run/42.log" "status.json log"
  ORCH_TMUX=0 ORCH_PROGRESS=0 orch_progress_set 42 done "ok" 2>/dev/null
  assert_eq "$(jq -c '.workers[0] | keys' "$TMP/repo/.orch/run/status.json")" '["cli","id","message","model","phase","phases","tier","updated_at"]' "default: no tmux fields"
fi
orch_tmux_kill_run
rc=0; tmux has-session -t "=$S:" 2>/dev/null || rc=$?
assert_eq "$rc" "1" "kill_run removes the run's sessions"

summary
