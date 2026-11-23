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

# Per-repo merge lock (Q4): sibling workers of one run, other runs and manual merges
# (scripts/merge-lock.sh) merge one at a time, so each merge re-checks freshness after
# the previous one landed. flock(2) on .orch/merge.lock: the kernel drops it when the
# holder dies, so there is no pid file, no stale lock and no takeover race. flock(1)
# when present; else (stock macOS) a perl helper holds flock(2) for this shell and
# exits on release or when this shell dies. Blocks up to ORCH_MERGE_LOCK_WAIT seconds
# (default ORCH_CI_TIMEOUT + 900, longer than one holder's post-rebase CI watch).
# Needs ORCH_REPO. Release with orch_merge_lock_release.
_ORCH_MERGE_LOCK_MODE="" _ORCH_MERGE_LOCK_PID=""
orch_merge_lock_acquire() {
  local lock="$ORCH_REPO/.orch/merge.lock" wait="${ORCH_MERGE_LOCK_WAIT:-$(( ${ORCH_CI_TIMEOUT:-1800} + 900 ))}"
  local lfd line=""
  mkdir -p "$ORCH_REPO/.orch"
  if command -v flock >/dev/null 2>&1; then
    exec 9>>"$lock"
    if flock -w "$wait" 9; then
      _ORCH_MERGE_LOCK_MODE=flock
      return 0
    fi
    exec 9>&-
    return 1
  fi
  if ! command -v perl >/dev/null 2>&1; then
    printf 'orch: the merge lock needs flock(1) or perl\n' >&2
    return 1
  fi
  coproc perl -MFcntl=:flock -e '
    my ($lock, $wait) = @ARGV;
    my $parent = getppid();
    open(my $fh, ">>", $lock) or exit 2;
    my $end = time + $wait;
    until (flock($fh, LOCK_EX | LOCK_NB)) { exit 1 if time >= $end; sleep 1; }
    $| = 1;
    print "locked\n";
    sleep 1 while getppid() == $parent;
  ' "$lock" "$wait" 8>&- 9>&-
  _ORCH_MERGE_LOCK_PID="$COPROC_PID"
  # Dup the read end now: bash unsets COPROC as soon as the helper exits.
  # (A bare `exec … 2>/dev/null` would silence this shell's stderr for good.)
  if { exec {lfd}<&"${COPROC[0]}"; } 2>/dev/null; then
    read -r line <&"$lfd" || line=""
    exec {lfd}<&-
  fi
  if [ "$line" = locked ]; then
    _ORCH_MERGE_LOCK_MODE=perl
    return 0
  fi
  wait "$_ORCH_MERGE_LOCK_PID" 2>/dev/null || true
  _ORCH_MERGE_LOCK_PID=""
  return 1
}

orch_merge_lock_release() {
  case "$_ORCH_MERGE_LOCK_MODE" in
    flock) exec 9>&- ;;
    perl)
      kill "$_ORCH_MERGE_LOCK_PID" 2>/dev/null || true
      wait "$_ORCH_MERGE_LOCK_PID" 2>/dev/null || true
      ;;
  esac
  _ORCH_MERGE_LOCK_MODE="" _ORCH_MERGE_LOCK_PID=""
}
