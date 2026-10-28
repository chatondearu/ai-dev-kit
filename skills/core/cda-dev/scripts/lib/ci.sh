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
    # No checks configured: green only with opt-in; else "none" (fail-fast, not pending).
    if [ "${ORCH_ALLOW_NO_CHECKS:-0}" = "1" ]; then
      printf 'green\n'
    else
      printf 'none\n'
    fi
    return 0
  fi
  if command -v jq >/dev/null 2>&1; then
    if ! jq -e 'type == "array"' <<<"$json" >/dev/null 2>&1; then
      printf 'pending\n'
      return 0
    fi
    jq -r --arg allow "${ORCH_ALLOW_NO_CHECKS:-0}" '
      if length == 0 then
        (if $allow == "1" then "green" else "none" end)
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

# Optional CI heartbeat into progress/status JSON (agent-pollable).
_orch_ci_heartbeat() {
  local pr="$1" status="$2" elapsed="$3"
  [ -n "${ORCH_WORKER_ID:-}" ] || return 0
  if declare -F orch_progress_set >/dev/null 2>&1; then
    orch_progress_set "$ORCH_WORKER_ID" ci "PR #${pr} ${status} (${elapsed}s)"
  fi
}

orch_pr_checks_watch() {
  local pr="$1"
  local timeout="${2:-600}"
  local start=$SECONDS
  local json status gh_rc
  local last_hb=-999
  local elapsed
  while true; do
    elapsed=$((SECONDS - start))
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
    if [ "$status" = "fail" ]; then
      printf 'red\n'
      return 1
    fi
    # Empty checks: do not wait for checks that will never appear.
    if [ "$status" = "none" ]; then
      printf 'none\n'
      printf 'orch: PR #%s has no checks; set ORCH_ALLOW_NO_CHECKS=1 or add CI (see doc/configuration.md)\n' "$pr" >&2
      return 1
    fi
    if (( elapsed - last_hb >= 30 )); then
      _orch_ci_heartbeat "$pr" "$status" "$elapsed"
      last_hb=$elapsed
    fi
    if (( elapsed >= timeout )); then
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
