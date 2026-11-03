#!/usr/bin/env bash
# shellcheck shell=bash
set -euo pipefail

# Hermetic: drop harness-exported ORCH_* so unit tests see a clean process.
# Individual tests re-export what they need (ORCH_TMUX, ORCH_ALLOW_NO_CHECKS, …).
while IFS= read -r _orch_k; do
  unset "$_orch_k" 2>/dev/null || true
done < <(env | awk -F= '/^ORCH_/ { print $1 }')
unset _orch_k

FAILS=0
PASSES=0

assert_eq() {
  local got="$1" want="$2" msg="${3:-}"
  if [ "$got" = "$want" ]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$msg" "$got" "$want" >&2
  fi
}

assert_contains() {
  local hay="$1" needle="$2" msg="${3:-}"
  if [[ "$hay" == *"$needle"* ]]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s\n  missing: %s\n  in: %s\n' "$msg" "$needle" "$hay" >&2
  fi
}

assert_file() {
  local path="$1" msg="${2:-file exists}"
  if [ -e "$path" ]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s: %s\n' "$msg" "$path" >&2
  fi
}

summary() {
  printf 'passes=%s fails=%s\n' "$PASSES" "$FAILS"
  [ "$FAILS" -eq 0 ]
}
