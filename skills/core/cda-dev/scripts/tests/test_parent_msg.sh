#!/usr/bin/env bash
# Parent cdamux msg on worker terminal + run end (#253 / ADR-011 §5).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/graph.sh"
source "$TROOT/../lib/progress.sh"
source "$TROOT/../lib/worktree.sh"

if ! command -v jq >/dev/null 2>&1; then
  echo "skip: jq not installed"
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
HOME_DIR="$TMP/cdamux-home"
REPO="$TMP/repo"
mkdir -p "$HOME_DIR" "$REPO"
export ORCH_REPO="$REPO" ORCH_PROGRESS=0 CDAMUX_HOME="$HOME_DIR" ORCH_TMUX=0
unset ORCH_RUN_ID CDAMUX_AGENT_ID
orch_test_stub_cdamux "$TMP"

inbox_count() {
  local f="$HOME_DIR/inbox/${1}.jsonl"
  [ -f "$f" ] || { echo 0; return; }
  wc -l <"$f" | tr -d ' '
}

# --- (1) worker terminal msgs when parent set ---
export CDAMUX_AGENT_ID="a-parent253"
orch_graph_init "run-msg-1"
ORCH_PROGRESS=0 orch_progress_set 42 done "PR #9 green" 2>/dev/null
assert_file "$HOME_DIR/inbox/a-parent253.jsonl" "worker done writes parent inbox"
assert_eq "$(inbox_count a-parent253)" "1" "one msg on first done"
assert_eq "$(jq -r .kind "$HOME_DIR/inbox/a-parent253.jsonl")" "done" "kind done"
assert_eq "$(jq -r .to "$HOME_DIR/inbox/a-parent253.jsonl")" "a-parent253" "to parent"
from42="$(cat "$REPO/.orch/run/42.agent_id")"
assert_eq "$(jq -r .from "$HOME_DIR/inbox/a-parent253.jsonl")" "$from42" "from worker agent"
assert_file "$REPO/.orch/run/run-msg-1.42.parent_msg" "idempotency marker run-scoped"

# retry same terminal → no duplicate
ORCH_PROGRESS=0 orch_progress_set 42 done "PR #9 green again" 2>/dev/null
assert_eq "$(inbox_count a-parent253)" "1" "done is at-most-once"

ORCH_PROGRESS=0 orch_progress_set 7 blocked "ci failed" 2>/dev/null
assert_eq "$(inbox_count a-parent253)" "2" "blocked adds second msg"
assert_eq "$(tail -n1 "$HOME_DIR/inbox/a-parent253.jsonl" | jq -r .kind)" "error" \
  "blocked maps to kind error"

# --- (2) run-end msg with integrator-report ref ---
orch_graph_finish ok
assert_eq "$(inbox_count a-parent253)" "3" "run end adds one msg"
last="$(tail -n1 "$HOME_DIR/inbox/a-parent253.jsonl")"
assert_eq "$(printf '%s' "$last" | jq -r .kind)" "report" "run-end kind report"
assert_contains "$(printf '%s' "$last" | jq -r .ref)" "integrator-report.md" "run-end ref"
assert_contains "$(printf '%s' "$last" | jq -r .text)" "run-msg-1" "run id in text"
orch_graph_finish ok
assert_eq "$(inbox_count a-parent253)" "3" "run-end at-most-once"

# --- (2b) new run same worker id → notify again (AFK / retry) ---
orch_graph_init "run-msg-2"
ORCH_PROGRESS=0 orch_progress_set 7 done "retry succeeded" 2>/dev/null
assert_eq "$(inbox_count a-parent253)" "4" "run2 done after run1 blocked sends again"
assert_file "$REPO/.orch/run/run-msg-2.7.parent_msg" "run2 marker distinct from run1"
assert_eq "$(tail -n1 "$HOME_DIR/inbox/a-parent253.jsonl" | jq -r .kind)" "done" \
  "run2 msg kind done"
orch_graph_finish ok
assert_eq "$(inbox_count a-parent253)" "5" "run2 end adds msg"

# --- (3) no-op without parent ---
unset CDAMUX_AGENT_ID
rm -f "$HOME_DIR/inbox"/*.jsonl
orch_graph_init "run-noparent"
ORCH_PROGRESS=0 orch_progress_set 99 done "ok" 2>/dev/null
orch_graph_finish ok
assert_eq "$(test -e "$HOME_DIR/inbox/a-parent253.jsonl" && echo yes || echo no)" "no" \
  "no inbox without CDAMUX_AGENT_ID"
assert_eq "$(ls "$HOME_DIR/inbox" 2>/dev/null | wc -l | tr -d ' ')" "0" "inbox empty without parent"

# --- (4) no-op without cdamux on PATH ---
export CDAMUX_AGENT_ID="a-parent253"
_clean=""
_old_ifs="$IFS"
IFS=:
for _d in $PATH; do
  [ -n "$_d" ] || continue
  [ -x "$_d/cdamux" ] && continue
  _clean="${_clean:+$_clean:}$_d"
done
IFS="$_old_ifs"
export PATH="$_clean"
unset _clean _old_ifs _d
! command -v cdamux >/dev/null 2>&1
rm -rf "$HOME_DIR/inbox"
mkdir -p "$HOME_DIR"
# clear markers so a buggy path would re-send
rm -f "$REPO/.orch/run"/*.parent_msg "$REPO/.orch/run"/*.parent_msg_run
orch_graph_init "run-nocli"
ORCH_PROGRESS=0 orch_progress_set 3 done "should not msg" 2>/dev/null
orch_graph_finish blocked
_inbox_n=0
[ -d "$HOME_DIR/inbox" ] && _inbox_n="$(ls -A "$HOME_DIR/inbox" 2>/dev/null | wc -l | tr -d ' ')"
assert_eq "$_inbox_n" "0" "no inbox writes without cdamux binary"
unset _inbox_n

summary
