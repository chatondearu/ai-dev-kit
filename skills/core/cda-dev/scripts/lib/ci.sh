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

# Prints green | red | none | timeout (M13: a timeout is not a red CI). rc 0 only on green.
orch_pr_checks_watch() {
  local pr="$1"
  local timeout="${2:-${ORCH_CI_TIMEOUT:-1800}}"
  # Seconds to keep waiting on "no checks reported" so a fresh push can register checks.
  local grace="${_ORCH_CI_NO_CHECKS_GRACE:-60}"
  local start=$SECONDS
  local json status gh_rc errf rc
  local last_hb=-999
  local elapsed
  errf="$(mktemp)"
  while true; do
    elapsed=$((SECONDS - start))
    gh_rc=0
    json="$(gh pr checks "$pr" --json name,state,bucket 2>"$errf")" || gh_rc=$?
    # Real gh on a PR without checks: empty stdout, exit 1, stderr
    # "no checks reported on the '<branch>' branch" — same as an empty list.
    if [ "$gh_rc" -ne 0 ] && grep -q 'no checks reported' "$errf" && (( elapsed >= grace || elapsed >= timeout )); then
      json="[]" gh_rc=0
    fi
    if [ "$gh_rc" -ne 0 ] || [ -z "$json" ]; then
      status="pending"
    else
      status="$(_orch_pr_checks_classify "$json")"
    fi
    if [ "$status" = "green" ]; then
      printf 'green\n'
      rc=0; break
    fi
    if [ "$status" = "fail" ]; then
      printf 'red\n'
      rc=1; break
    fi
    # Empty checks: do not wait for checks that will never appear.
    if [ "$status" = "none" ]; then
      printf 'none\n'
      printf 'orch: PR #%s has no checks; set ORCH_ALLOW_NO_CHECKS=1 or add CI (see doc/configuration.md)\n' "$pr" >&2
      rc=1; break
    fi
    if (( elapsed - last_hb >= 30 )); then
      _orch_ci_heartbeat "$pr" "$status" "$elapsed"
      last_hb=$elapsed
    fi
    if (( elapsed >= timeout )); then
      printf 'timeout\n'
      rc=1; break
    fi
    sleep 5
  done
  rm -f "$errf"
  return "$rc"
}

# Prints the PR's mergeable state; rc 0 only for an OPEN, MERGEABLE PR. GitHub computes
# it lazily (UNKNOWN right after a push or a sibling merge): re-ask every 5 s for up to
# ORCH_MERGEABLE_WAIT seconds (default 60), then refuse UNKNOWN (Q4).
orch_pr_mergeable() {
  local pr="$1" wait="${ORCH_MERGEABLE_WAIT:-60}" start=$SECONDS
  local json m state
  while :; do
    json="$(gh pr view "$pr" --json mergeable,state 2>/dev/null)" || return 1
    if command -v jq >/dev/null 2>&1; then
      m="$(jq -r '.mergeable // empty' <<<"$json")"
      state="$(jq -r '.state // empty' <<<"$json")"
    else
      m="$(echo "$json" | sed -n 's/.*"mergeable"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
      state="$(echo "$json" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
    fi
    [ -z "$m" ] && m="UNKNOWN"
    [ "$m" = UNKNOWN ] && (( SECONDS - start < wait )) || break
    sleep 5
  done
  printf '%s\n' "$m"
  if [ -n "$state" ] && [ "$state" != "OPEN" ]; then
    return 1
  fi
  [ "$m" = MERGEABLE ]
}

# First PR state for branch head: OPEN|MERGED|CLOSED| (empty if none). A failed gh
# lookup prints UNKNOWN (rc 1), never "no PR" (L11); callers only act on MERGED.
orch_pr_state() {
  local branch="$1" json
  [ -n "$branch" ] || { printf '\n'; return 0; }
  command -v gh >/dev/null 2>&1 || { printf '\n'; return 0; }
  json="$(gh pr list --head "$branch" --state all --json number,state 2>/dev/null)" || {
    printf 'UNKNOWN\n'
    return 1
  }
  if command -v jq >/dev/null 2>&1; then
    jq -r '.[0].state // empty' <<<"$json" 2>/dev/null || true
    return 0
  fi
  printf '%s\n' "$(echo "$json" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
}

# JSON array of OPEN|MERGED PRs for branch (empty array / "" on failure). Missing state → OPEN.
orch_pr_list_open_or_merged() {
  local branch="$1" json
  [ -n "$branch" ] || { printf '[]\n'; return 0; }
  command -v gh >/dev/null 2>&1 || { printf '[]\n'; return 0; }
  json="$(gh pr list --head "$branch" --state all --json number,url,state,title 2>/dev/null)" || {
    printf '[]\n'
    return 0
  }
  [ -n "$json" ] || { printf '[]\n'; return 0; }
  if command -v jq >/dev/null 2>&1; then
    jq -c '[.[] | select((.state // "OPEN") == "OPEN" or .state == "MERGED")]' <<<"$json" 2>/dev/null \
      || printf '[]\n'
    return 0
  fi
  # No jq: keep OPEN|MERGED only (missing state → OPEN); CLOSED-only → [].
  local out="" obj st
  while IFS= read -r obj; do
    [ -n "$obj" ] || continue
    st="$(printf '%s' "$obj" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
    [ -z "$st" ] && st="OPEN"
    case "$st" in
      OPEN|MERGED)
        [ -n "$out" ] && out+=","
        out+="$obj"
        ;;
    esac
  done < <(printf '%s' "$json" | grep -oE '\{[^{}]*\}' || true)
  if [ -n "$out" ]; then
    printf '[%s]\n' "$out"
  else
    printf '[]\n'
  fi
}
