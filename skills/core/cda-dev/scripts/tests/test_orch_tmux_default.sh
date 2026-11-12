#!/usr/bin/env bash
# ORCH_TMUX defaults to 1 (ADR-011); --sandbox / ORCH_TMUX=0 stays headless.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"

# helpers.sh forces ORCH_TMUX=0 for unit tests; probe the production default.
unset ORCH_TMUX
assert_eq "${ORCH_TMUX:-1}" "1" "default ORCH_TMUX is 1"
# orch_tmux_on still needs tmux + not sandbox.
export ORCH_SANDBOX=0
if command -v tmux >/dev/null 2>&1; then
  unset ORCH_TMUX
  orch_tmux_on
  assert_eq "$?" "0" "orch_tmux_on true when default + tmux present"
fi
export ORCH_TMUX=0
orch_tmux_on && rc=0 || rc=$?
assert_eq "$rc" "1" "ORCH_TMUX=0 disables"
export ORCH_TMUX=1 ORCH_SANDBOX=1
orch_tmux_on && rc=0 || rc=$?
assert_eq "$rc" "1" "sandbox disables tmux even when ORCH_TMUX=1"

# Help text documents the default.
help="$(bash "$TROOT/../orch.sh" --help)"
assert_contains "$help" "ORCH_TMUX=1" "orch.sh --help mentions ORCH_TMUX=1 default"

# --sandbox: warn only when the caller explicitly set ORCH_TMUX=1.
err_explicit="$(ORCH_TMUX=1 bash "$TROOT/../orch.sh" --repo /tmp --mode intake --sandbox 2>&1 || true)"
assert_contains "$err_explicit" "ORCH_TMUX ignored with --sandbox" "warn when ORCH_TMUX=1 + sandbox"
err_implicit="$(unset ORCH_TMUX; bash "$TROOT/../orch.sh" --repo /tmp --mode intake --sandbox 2>&1 || true)"
if [[ "$err_implicit" == *"ORCH_TMUX ignored with --sandbox"* ]]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL sandbox alone must not warn about ORCH_TMUX\n  got: %s\n' "$err_implicit" >&2
else
  PASSES=$((PASSES + 1))
fi

summary
