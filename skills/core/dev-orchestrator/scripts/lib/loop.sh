#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). Verification loop runner for workers.

orch_loop_worker() {
  local wt="$1" prompt="$2"
  local max="${ORCH_MAX_ITER:-10}" i=1
  while [ "$i" -le "$max" ]; do
    printf 'orch loop iter %s/%s\n' "$i" "$max" >&2
    orch_agent_run "$wt" "$prompt" || true
    if orch_run_verify "$wt"; then
      printf 'verify ok\n' >&2
      return 0
    fi
    i=$((i + 1))
  done
  printf 'blocked\n' >&2
  return 1
}
