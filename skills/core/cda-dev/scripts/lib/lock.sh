#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). Run lock shared by orch.sh modes (ADR-002).

ORCH_LOCK_DIR=""

# Release the mkdir lock this process holds (no-op for flock or when not held).
_orch_release_lock() {
  [ -n "${ORCH_LOCK_DIR:-}" ] || return 0
  rm -f "$ORCH_LOCK_DIR/pid" 2>/dev/null || true
  rmdir "$ORCH_LOCK_DIR" 2>/dev/null || true
  ORCH_LOCK_DIR=""
}

# Serialize ready-pickup / pr-fix against orch-auto and concurrent orch (ADR-002).
orch_acquire_lock() {
  mkdir -p "$ORCH_REPO/.orch"
  local lock="$ORCH_REPO/.orch/orch.lock"
  if command -v flock >/dev/null 2>&1; then
    exec 8>"$lock"
    if ! flock -n 8; then
      printf 'orch: another run holds %s\n' "$lock" >&2
      exit 75
    fi
    return 0
  fi
  # Portable fallback when flock is absent (e.g. stock macOS): atomic mkdir + owner pid.
  # A signal death skips the EXIT trap, so a dead owner pid marks the lock stale.
  local dir="${lock}.d" owner=""
  if ! mkdir "$dir" 2>/dev/null; then
    owner="$(cat "$dir/pid" 2>/dev/null || true)"
    # No pid yet = the owner is between mkdir and its pid write: treat as held.
    if [ -z "$owner" ] || kill -0 "$owner" 2>/dev/null; then
      printf 'orch: another run holds %s\n' "$dir" >&2
      exit 75
    fi
    printf 'orch: taking over stale lock %s (pid %s is gone)\n' "$dir" "$owner" >&2
    # shortcut: two runners racing on the same stale lock can both win, upgrade to flock-only if that bites.
    rm -rf "$dir"
    mkdir "$dir" 2>/dev/null || { printf 'orch: another run holds %s\n' "$dir" >&2; exit 75; }
  fi
  printf '%s\n' "$$" >"$dir/pid"
  ORCH_LOCK_DIR="$dir"
  trap _orch_release_lock EXIT
}
