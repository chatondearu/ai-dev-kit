#!/usr/bin/env bash
# Q12 / H9: an interrupted or crashed orch run is marked blocked, the parent inbox
# gets kind=error, the agent is stopped and the run lock is released.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(cd "$(mktemp -d)" && pwd -P)"
STUB_BIN="$(mktemp -d)"
_ORIG_PATH="$PATH"
orch_pid=""
cleanup() {
  PATH="$_ORIG_PATH"
  [ -z "$orch_pid" ] || kill -KILL -- "-$orch_pid" 2>/dev/null || true
  git -C "$TMP/repo" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

R="$TMP/repo"
mkdir -p "$R"
git -C "$R" init -q -b main
git -C "$R" config user.email t@t
git -C "$R" config user.name t
echo init >"$R/README"
git -C "$R" add README
git -C "$R" commit -qm init
git -C "$R" branch -q orch/pr-77

orch_test_stub_cdamux "$TMP"
export CDAMUX_HOME="$TMP/cdamux" CDAMUX_AGENT_ID="a-parent" ORCH_GRAPH=1 ORCH_TMUX=0 ORCH_PROGRESS=0
export ORCH_VERIFY=true ORCH_ALLOW_NO_CHECKS=1 ORCH_GRAPH_VERIFY=0 ORCH_MIN_FREE_GIB=0

MARK="orch-trap-sleeper-$$"
cat >"$STUB_BIN/$MARK" <<EOF
#!/usr/bin/env bash
cat >/dev/null
touch "$TMP/agent-started"
while :; do sleep 1; done
EOF
chmod +x "$STUB_BIN/$MARK"

cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"pr view"*headRefName*) echo orch/pr-77 ;;
  *"pr view"*isCrossRepository*) echo false ;;
  *"repo view"*defaultBranchRef*) echo main ;;
  *"pr view"*reviews*) echo '{"commits":[{"committedDate":"2026-11-10T10:00:00Z"}],"reviews":[{"author":{"login":"o"},"authorAssociation":"OWNER","state":"CHANGES_REQUESTED","submittedAt":"2026-11-10T11:00:00Z","body":"FIX"}],"comments":[]}' ;;
  *"api graphql"*) echo '{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[]}}}}}' ;;
  *"pr checkout"*) git checkout -q orch/pr-77 ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"

inbox="$CDAMUX_HOME/inbox/a-parent.jsonl"

echo "# SIGTERM to the process group mid-run (Ctrl-C sends INT the same way)"
export ORCH_RUN_ID="trap-term-1"
set -m
bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr 77 --agent "$STUB_BIN/$MARK" >"$TMP/term.log" 2>&1 &
orch_pid=$!
set +m
for _ in $(seq 100); do [ -f "$TMP/agent-started" ] && break; sleep 0.1; done
assert_file "$TMP/agent-started" "agent started before the signal"
# Bash runs a trap only after its foreground child exits: signal the group, as a tty does.
# TERM, not INT: async jobs (verify run in the background) start with INT ignored.
kill -TERM -- "-$orch_pid"
rc=0
for _ in $(seq 100); do kill -0 "$orch_pid" 2>/dev/null || break; sleep 0.1; done
wait "$orch_pid" 2>/dev/null || rc=$?
orch_pid=""
assert_eq "$rc" "130" "orch exits 130 on SIGTERM"
sleep 0.3
if pgrep -f "$MARK" >/dev/null; then
  FAILS=$((FAILS + 1)); printf 'FAIL agent survived SIGTERM\n' >&2
  pkill -KILL -f "$MARK" || true
else
  PASSES=$((PASSES + 1)); printf 'ok   agent stopped\n'
fi
gf="$CDAMUX_HOME/runs/trap-term-1.json"
assert_file "$gf" "run JSON written"
assert_eq "$(jq -r .status "$gf")" "blocked" "interrupted run is blocked, not running"
assert_eq "$([ -n "$(jq -r '.ended_at // empty' "$gf")" ] && echo yes)" "yes" "interrupted run has ended_at"
msg="$(jq -c 'select(.text | contains("trap-term-1"))' "$inbox" 2>/dev/null | tail -n 1)"
assert_eq "$(jq -r .kind <<<"$msg")" "error" "parent inbox gets kind error"
assert_contains "$(jq -r .text <<<"$msg")" "interrupted by SIGTERM" "parent message names the signal"
if [ -e "$R/.orch/orch.lock.d" ]; then
  FAILS=$((FAILS + 1)); printf 'FAIL run lock left behind\n' >&2
else
  PASSES=$((PASSES + 1)); printf 'ok   run lock released\n'
fi

echo "# crash (set -e) after the lock"
export ORCH_RUN_ID="trap-crash-1"
git -C "$R" worktree remove --force "$(orch_worktree_path "$R" pr-77 fix)" 2>/dev/null || true
rm -rf "$R/.orch/worktrees"
: >"$R/.orch/worktrees" # mkdir -p of the worktree parent now fails
set +e
bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr 77 --agent "$STUB_BIN/$MARK" >"$TMP/crash.log" 2>&1
rc=$?
set -e
assert_eq "$([ "$rc" -ne 0 ] && echo nonzero)" "nonzero" "crash exits non-zero"
msg="$(jq -c 'select(.text | contains("trap-crash-1"))' "$inbox" 2>/dev/null | tail -n 1)"
assert_eq "$(jq -r .kind <<<"$msg")" "error" "crash: parent inbox gets kind error"
assert_contains "$(jq -r .text <<<"$msg")" "exited (code 1)" "crash: message names the exit code"
rm -f "$R/.orch/worktrees"

echo "# normal blocked run: one report, no extra error"
before="$(wc -l <"$inbox" | tr -d ' ')"
export ORCH_RUN_ID="trap-none-1"
set +e
bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr 77 --agent false >"$TMP/none.log" 2>&1
set -e
after="$(jq -c 'select(.text | contains("trap-none-1"))' "$inbox" | jq -r .kind | paste -sd, -)"
assert_eq "$after" "report" "finished blocked run sends one report, not an error" || cat "$TMP/none.log" >&2
: "$before"

summary
