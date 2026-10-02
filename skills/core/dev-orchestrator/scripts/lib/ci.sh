#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). Never merges PRs.

_orch_pr_checks_classify() {
  local json="$1"
  if [ -z "$json" ]; then
    printf 'pending\n'
    return 0
  fi
  if [ "$json" = "[]" ]; then
    printf 'green\n'
    return 0
  fi
  if command -v jq >/dev/null 2>&1; then
    if ! jq -e 'type == "array"' <<<"$json" >/dev/null 2>&1; then
      printf 'pending\n'
      return 0
    fi
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
  local json status gh_rc
  while true; do
    gh_rc=0
    json="$(gh pr checks "$pr" --json name,state,bucket 2>/dev/null)" || gh_rc=$?
    if [ "$gh_rc" -ne 0 ] || [ -z "$json" ]; then
      status="pending"
    else
      status="$(_orch_pr_checks_classify "$json")"
    fi
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

orch_pr_mergeable() {
  local pr="$1"
  local json m state
  json="$(gh pr view "$pr" --json mergeable,state 2>/dev/null)" || return 1
  if command -v jq >/dev/null 2>&1; then
    m="$(jq -r '.mergeable // empty' <<<"$json")"
    state="$(jq -r '.state // empty' <<<"$json")"
  else
    m="$(echo "$json" | sed -n 's/.*"mergeable"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
    state="$(echo "$json" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
  fi
  [ -z "$m" ] && m="UNKNOWN"
  printf '%s\n' "$m"
  if [ -n "$state" ] && [ "$state" != "OPEN" ]; then
    return 1
  fi
  case "$m" in
    MERGEABLE|UNKNOWN) return 0 ;;
    *) return 1 ;;
  esac
}
