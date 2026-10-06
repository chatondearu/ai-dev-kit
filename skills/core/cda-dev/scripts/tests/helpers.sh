#!/usr/bin/env bash
# shellcheck shell=bash
set -euo pipefail

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
