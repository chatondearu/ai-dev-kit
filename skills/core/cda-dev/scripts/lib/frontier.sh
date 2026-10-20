#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).
#
# Frontier gating: a Ready child is dispatchable only when every issue it
# declares as a blocker is already closed. Blockers are recorded on the child
# body with a line:
#
#     Blocked by: #12, #15
#
# This is the same dependency language cda-explore uses, so a decision that
# graduates into implementation keeps one blocking convention end to end.

# Pure parser: echo the space-separated blocker issue numbers found on the
# first `Blocked by:` line of an issue body. Case-insensitive; tolerates
# `#12`, `12`, commas and/or spaces. No network. Echoes nothing when absent.
orch_parse_blockers() {
  local body="$1" line nums
  line="$(printf '%s\n' "$body" \
    | grep -iE '^[[:space:]]*blocked[[:space:]]+by[[:space:]]*:' | head -n 1 || true)"
  nums="$(printf '%s' "$line" | grep -oE '[0-9]+' || true)"
  printf '%s' "$nums" | awk '!seen[$0]++' | tr '\n' ' ' | sed 's/[[:space:]]*$//'
}

# gh wrapper: echo OPEN or CLOSED for an issue. On any gh error, echo OPEN so a
# declared-but-unverifiable blocker is held (safe) rather than silently cleared.
orch_issue_state() {
  local repo="$1" num="$2" state
  state="$(gh issue view "$num" --repo "$repo" --json state -q .state 2>/dev/null || true)"
  case "$state" in
    CLOSED) printf 'CLOSED\n' ;;
    *) printf 'OPEN\n' ;;
  esac
}

# Partition issue numbers into the dispatchable frontier and the held set.
# Sets three global arrays for the caller:
#   FRONTIER_IDS  — ids whose blockers are all closed (or that declare none)
#   HELD_IDS      — ids with at least one open blocker
#   HELD_REASON   — parallel to HELD_IDS: the first open blocker number
# Fail-closed on a body-fetch error (can't verify blockers → hold), so a flaky
# gh never silently clears dependencies. Empty body with successful fetch is OK.
orch_frontier_partition() {
  local repo="$1"; shift
  FRONTIER_IDS=(); HELD_IDS=(); HELD_REASON=()
  local id body blockers b open_blocker raw gh_rc
  for id in "$@"; do
    gh_rc=0
    raw="$(gh issue view "$id" --repo "$repo" --json body 2>/dev/null)" || gh_rc=$?
    if [ "$gh_rc" -ne 0 ] || [ -z "$raw" ]; then
      HELD_IDS+=("$id"); HELD_REASON+=("body-fetch-failed")
      continue
    fi
    body="$(printf '%s' "$raw" | jq -r '.body // empty' 2>/dev/null || true)"
    blockers="$(orch_parse_blockers "$body")"
    open_blocker=""
    for b in $blockers; do
      if [ "$(orch_issue_state "$repo" "$b")" != "CLOSED" ]; then
        open_blocker="$b"; break
      fi
    done
    if [ -n "$open_blocker" ]; then
      HELD_IDS+=("$id"); HELD_REASON+=("$open_blocker")
    else
      FRONTIER_IDS+=("$id")
    fi
  done
}
