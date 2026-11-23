#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). Run lock shared by orch.sh modes (ADR-002).

ORCH_LOCK_DIR=""

# Release the mkdir lock this process holds (no-op for flock or when not held).
_orch_release_lock() {
  [ -n "${ORCH_LOCK_DIR:-}" ] || return 0
  rm -f "$ORCH_LOCK_DIR/pid" "$ORCH_LOCK_DIR/run" 2>/dev/null || true
  rmdir "$ORCH_LOCK_DIR" 2>/dev/null || true
  ORCH_LOCK_DIR=""
}

# Serialize ready-pickup / pr-fix against orch-auto and concurrent orch (ADR-002).
# Usage: orch_acquire_lock [lock-file]  (default <repo>/.orch/orch.lock; orch-auto passes auto.lock)
# Exit 75 = held: the caller reports "lock held by run <id>" and never spawns
# duplicate workers for the same issues (Q11, H7).
orch_acquire_lock() {
  mkdir -p "$ORCH_REPO/.orch"
  local lock="${1:-$ORCH_REPO/.orch/orch.lock}"
  if command -v flock >/dev/null 2>&1; then
    exec 8>"$lock"
    if ! flock -n 8; then
      printf 'orch: lock held by run %s (%s)\n' "$(cat "$lock.run" 2>/dev/null || echo unknown)" "$lock" >&2
      exit 75
    fi
    printf '%s\n' "${ORCH_RUN_ID:-unknown}" >"$lock.run"
    return 0
  fi
  # Portable fallback when flock is absent (e.g. stock macOS): atomic mkdir + owner pid.
  # A signal death skips the EXIT trap, so a dead owner pid marks the lock stale.
  local dir="${lock}.d" owner=""
  if ! mkdir "$dir" 2>/dev/null; then
    owner="$(cat "$dir/pid" 2>/dev/null || true)"
    # No pid yet = the owner is between mkdir and its pid write: treat as held.
    if [ -z "$owner" ] || kill -0 "$owner" 2>/dev/null; then
      printf 'orch: lock held by run %s (%s)\n' "$(cat "$dir/run" 2>/dev/null || echo unknown)" "$dir" >&2
      exit 75
    fi
    printf 'orch: taking over stale lock %s (pid %s is gone)\n' "$dir" "$owner" >&2
    # shortcut: two runners racing on the same stale lock can both win, upgrade to flock-only if that bites.
    rm -rf "$dir"
    mkdir "$dir" 2>/dev/null || { printf 'orch: lock held by run unknown (%s)\n' "$dir" >&2; exit 75; }
  fi
  printf '%s\n' "$$" >"$dir/pid"
  printf '%s\n' "${ORCH_RUN_ID:-unknown}" >"$dir/run"
  ORCH_LOCK_DIR="$dir"
  trap _orch_release_lock EXIT
}

# Per-repo merge lock (Q4): sibling workers of one run and other runs merge one at a
# time, so each merge re-checks freshness after the previous one landed. Blocks up to
# ORCH_MERGE_LOCK_WAIT seconds (default: ORCH_CI_TIMEOUT + 900, longer than one holder's
# post-rebase CI watch). Release with orch_merge_lock_release.
_ORCH_MERGE_LOCK_MODE=""
orch_merge_lock_acquire() {
  local lock="$ORCH_REPO/.orch/merge.lock" wait="${ORCH_MERGE_LOCK_WAIT:-$(( ${ORCH_CI_TIMEOUT:-1800} + 900 ))}" start=$SECONDS owner
  mkdir -p "$ORCH_REPO/.orch"
  if command -v flock >/dev/null 2>&1; then
    exec 9>"$lock"
    if flock -w "$wait" 9; then
      _ORCH_MERGE_LOCK_MODE=flock
      return 0
    fi
    exec 9>&-
    return 1
  fi
  while ! mkdir "$lock.d" 2>/dev/null; do
    owner="$(cat "$lock.d/pid" 2>/dev/null || true)"
    # Empty pid = owner between mkdir and pid write: held. Dead pid = stale.
    if [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; then
      rm -rf "$lock.d"
      continue
    fi
    (( SECONDS - start < wait )) || return 1
    sleep 1
  done
  printf '%s\n' "$BASHPID" >"$lock.d/pid"
  _ORCH_MERGE_LOCK_MODE=mkdir
}

orch_merge_lock_release() {
  case "$_ORCH_MERGE_LOCK_MODE" in
    flock) exec 9>&- ;;
    mkdir) rm -rf "$ORCH_REPO/.orch/merge.lock.d" ;;
  esac
  _ORCH_MERGE_LOCK_MODE=""
}
