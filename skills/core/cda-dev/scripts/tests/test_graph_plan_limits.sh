#!/usr/bin/env bash
# graph-plan: max_parallel cap is honored; files outside a task's `paths` block it.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

source "$TROOT/../lib/worktree.sh"
source "$TROOT/../lib/progress.sh"
source "$TROOT/../lib/sandbox.sh"
source "$TROOT/../lib/loop.sh"
source "$TROOT/../lib/graph.sh"

TMP="$(mktemp -d)"
trap 'git -C "$TMP" worktree prune 2>/dev/null || true; rm -rf "$TMP"' EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email t@test
git -C "$TMP" config user.name t
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init

export ORCH_REPO="$TMP" ORCH_PROMPTS_DIR=/nonexistent ORCH_PROGRESS=0
export STUB_LOG="$TMP/worker.log"

# Stub worker: log start/end markers, optionally write STUB_WRITE (relative to the worktree).
orch_graph_worker() {
  local wt="$1"
  echo "start $ORCH_WORKER_ID" >>"$STUB_LOG"
  sleep 0.3
  if [ -n "${STUB_WRITE:-}" ]; then
    mkdir -p "$wt/$(dirname "$STUB_WRITE")"
    echo x >"$wt/$STUB_WRITE"
  fi
  echo "end $ORCH_WORKER_ID" >>"$STUB_LOG"
}
export -f orch_graph_worker

node_msg() { jq -r --arg id "$1" '.nodes[] | select(.id == $id) | "\(.status) \(.message)"' "$(orch_graph_file)"; }

# (a) three tasks, cap 1 → start/end markers never interleave.
PLAN3="$TMP/plan3.json"
cat >"$PLAN3" <<'EOF'
{"issue":11,"tasks":[
  {"id":"a","paths":["src/a/"],"prompt":"a"},
  {"id":"b","paths":["src/b/"],"prompt":"b"},
  {"id":"c","paths":["src/c/"],"prompt":"c"}]}
EOF
: >"$STUB_LOG"
ORCH_MAX_PARALLEL=1 orch_graph_plan_run "$PLAN3" 11 2>/dev/null
assert_eq "$(grep -c '^end' "$STUB_LOG")" "3" "cap 1: all three tasks ran"
assert_eq "$(awk '{print $1}' "$STUB_LOG" | paste -sd' ' -)" "start end start end start end" "cap 1: no overlap"

# Control: cap 3 does overlap (proves the check above can fail).
sed 's/"issue":11/"issue":12/' "$PLAN3" >"$TMP/plan3b.json"
: >"$STUB_LOG"
ORCH_MAX_PARALLEL=3 orch_graph_plan_run "$TMP/plan3b.json" 12 2>/dev/null
assert_eq "$(head -n3 "$STUB_LOG" | awk '{print $1}' | paste -sd' ' -)" "start start start" "cap 3: tasks overlap"

# (b) file outside paths → blocked, file named in the reason.
PLAN1="$TMP/plan1.json"
echo '{"issue":13,"tasks":[{"id":"a","paths":["src/a/"],"prompt":"a"}]}' >"$PLAN1"
: >"$STUB_LOG"
rc=0
STUB_WRITE="src/other/leak.txt" orch_graph_plan_run "$PLAN1" 13 2>/dev/null || rc=$?
assert_eq "$rc" "1" "outside paths: plan fails"
assert_contains "$(node_msg task-a)" "blocked paths: src/other/leak.txt" "outside paths: node blocked with file"

# (c) file inside paths → done.
echo '{"issue":14,"tasks":[{"id":"a","paths":["src/a/"],"prompt":"a"}]}' >"$PLAN1"
rc=0
STUB_WRITE="src/a/ok.txt" orch_graph_plan_run "$PLAN1" 14 2>/dev/null || rc=$?
assert_eq "$rc" "0" "inside paths: plan ok"
assert_eq "$(node_msg task-a)" "done complete" "inside paths: node done"

summary
