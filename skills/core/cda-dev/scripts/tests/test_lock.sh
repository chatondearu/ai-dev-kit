#!/usr/bin/env bash
# orch run lock, mkdir fallback (no flock): stale pid takeover, live pid -> 75,
# TERM mid-run releases the lock; the same run proves the worker prompt names its branch.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"

TMP="$(mktemp -d)"
STUB="$(mktemp -d)"
NOFLOCK="$(mktemp -d)"
_ORIG_PATH="$PATH"
cleanup() {
  PATH="$_ORIG_PATH"
  [ -z "${RUN_PID:-}" ] || kill "$RUN_PID" 2>/dev/null || true
  git -C "$TMP" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB" "$NOFLOCK"
}
trap cleanup EXIT

# Hide flock when the host has it (stock macOS does not): shadow every other tool.
if command -v flock >/dev/null 2>&1; then
  IFS=: read -ra dirs <<<"$PATH"
  for d in "${dirs[@]}"; do
    for f in "$d"/*; do
      # First match wins, like PATH lookup (a later xcrun `git` shim must not shadow nix git).
      [ -x "$f" ] && [ "${f##*/}" != flock ] && [ ! -e "$NOFLOCK/${f##*/}" ] && ln -s "$f" "$NOFLOCK/${f##*/}"
    done
  done
  PATH="$NOFLOCK"
fi
! command -v flock >/dev/null 2>&1 || { echo "FAIL flock still visible" >&2; exit 1; }

git -C "$TMP" init -q
git -C "$TMP" config user.email t@t
git -C "$TMP" config user.name t
mkdir -p "$TMP/plans"
echo x >"$TMP/plans/PLAN-7-lock.md"
git -C "$TMP" add plans
git -C "$TMP" commit -qm init
LOCKD="$TMP/.orch/orch.lock.d"

acquire() { # run orch_acquire_lock in a child shell; echo its pid then exit code
  bash -c 'source "$1/lib/lock.sh"; ORCH_REPO="$2"; orch_acquire_lock; echo "pid=$$"; cat "$ORCH_LOCK_DIR/pid"' _ "$SCRIPTS" "$TMP" 2>"$TMP/err"
}

echo "# fresh lock: pid recorded, dir removed on exit"
out="$(acquire)"
assert_contains "$out" "pid=" "acquired"
assert_eq "$(printf '%s\n' "$out" | sed -n 1p | cut -d= -f2)" "$(printf '%s\n' "$out" | sed -n 2p)" "pid file holds the owner pid"
[ ! -e "$LOCKD" ] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL lock dir left after normal exit" >&2; }

echo "# stale lock (dead pid) is taken over"
sleep 0 & dead=$!; wait "$dead"
mkdir -p "$LOCKD"; echo "$dead" >"$LOCKD/pid"
set +e; out="$(acquire)"; rc=$?; set -e
assert_eq "$rc" "0" "stale lock acquired"
assert_contains "$(cat "$TMP/err")" "taking over stale lock" "logs the takeover"

echo "# live pid -> exit 75, lock untouched"
sleep 30 & live=$!
mkdir -p "$LOCKD"; echo "$live" >"$LOCKD/pid"
set +e; out="$(acquire)"; rc=$?; set -e
kill "$live" 2>/dev/null || true
assert_eq "$rc" "75" "live owner -> 75"
assert_contains "$(cat "$TMP/err")" "another run holds" "reports contention"
assert_eq "$(cat "$LOCKD/pid")" "$live" "live owner's pid file kept"
rm -rf "$LOCKD"

echo "# TERM during a run releases the lock; live run blocks a second one"
cat >"$STUB/gh" <<'GH'
#!/usr/bin/env bash
exit 0
GH
cat >"$STUB/stub-agent" <<AG
#!/usr/bin/env bash
cat >/dev/null
touch "$TMP/agent-started"
sleep 30
AG
chmod +x "$STUB/gh" "$STUB/stub-agent"
PATH="$STUB:$PATH"
export ORCH_VERIFY=true ORCH_PROGRESS=0 ORCH_FRONTIER=0 ORCH_ALLOW_NO_CHECKS=1
ORCH_ARGS=(--repo "$TMP" --mode ready-pickup --issue 7 --agent "$STUB/stub-agent" --no-progress)

set -m # own process group: orch's `kill 0` must not reach this test
bash "$SCRIPTS/orch.sh" "${ORCH_ARGS[@]}" >"$TMP/run.log" 2>&1 &
RUN_PID=$!
set +m
for _ in $(seq 1 100); do [ -e "$TMP/agent-started" ] && break; sleep 0.1; done
assert_file "$TMP/agent-started" "worker agent started"
assert_eq "$(cat "$LOCKD/pid")" "$RUN_PID" "running orch holds the lock"
assert_contains "$(cat "$TMP/.orch/worktrees/7-issue-7/.orch/worker-prompt.md")" 'Branch: `feat/7-issue-7`' "worker prompt names the PR branch"
set +e; bash "$SCRIPTS/orch.sh" "${ORCH_ARGS[@]}" >"$TMP/second.log" 2>&1; rc=$?; set -e
assert_eq "$rc" "75" "second run exits 75 while the first lives"

kill -TERM "$RUN_PID"
wait "$RUN_PID" 2>/dev/null || true
RUN_PID=""
[ ! -e "$LOCKD" ] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL lock dir left after TERM" >&2; }

summary
