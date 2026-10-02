#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). Never merges PRs.

_orch_pr_checks_classify() {
  local json="$1"
  if [ -z "$json" ] || [ "$json" = "[]" ]; then
    printf 'green\n'
    return 0
  fi
  if command -v jq >/dev/null 2>&1; then
    jq -r '
      if length == 0 then "green"
      elif any(.bucket == "fail" or .state == "FAILURE") then "fail"
      elif any(
        .bucket == "pending"
        or .state == "PENDING"
        or .state == "IN_PROGRESS"
        or .state == "QUEUED"
      ) then "pending"
      elif all(.bucket == "pass") then "green"
      else "pending"
      end
    ' <<<"$json"
    return 0
  fi
  if echo "$json" | grep -Eq '"bucket":"fail"|"state":"FAILURE"'; then
    printf 'fail\n'
    return 0
  fi
  if echo "$json" | grep -Eq '"bucket":"pending"|"state":"PENDING"|"state":"IN_PROGRESS"|"state":"QUEUED"'; then
    printf 'pending\n'
    return 0
  fi
  if echo "$json" | grep -q '"bucket":"pass"' && ! echo "$json" | grep -qv '"bucket":"pass"'; then
    printf 'green\n'
    return 0
  fi
  printf 'pending\n'
}

orch_pr_checks_watch() {
  local pr="$1"
  local timeout="${2:-600}"
  local start=$SECONDS
  local json status
  while true; do
    json="$(gh pr checks "$pr" --json name,state,bucket 2>/dev/null || true)"
    status="$(_orch_pr_checks_classify "$json")"
    if [ "$status" = "green" ]; then
      printf 'green\n'
      return 0
    fi
    if [ "$status" = "fail" ] && (( SECONDS - start >= timeout )); then
      printf 'red\n'
      return 1
    fi
    if (( SECONDS - start >= timeout )); then
      printf 'pending\n'
      return 1
    fi
    sleep 5
  done
}
