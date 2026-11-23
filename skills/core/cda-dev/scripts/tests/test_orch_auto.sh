#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"

TMP="$(mktemp -d)"
STUB_BIN="$(mktemp -d)"
_ORIG_PATH="$PATH"

cleanup() {
  PATH="$_ORIG_PATH"
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

if ! command -v jq >/dev/null 2>&1; then
  echo "SKIP test_orch_auto.sh (jq not installed)"
  exit 0
fi

git -C "$TMP" init -q
git -C "$TMP" config user.email auto@test
git -C "$TMP" config user.name auto
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init

cat >"$STUB_BIN/fake-gh-board.sh" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
  list-ready) printf '10\n11\n12\n' ;;
  *) echo "unexpected: $*" >&2; exit 1 ;;
esac
EOF
chmod +x "$STUB_BIN/fake-gh-board.sh"

cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"repo view"*) ;;
  *"issue view"*)
    [ "${GH_STUB_FAIL_LABELS:-0}" = 1 ] && { echo "API rate limit exceeded" >&2; exit 1; } ;;
  *"pr list"*)
    [ "${GH_STUB_FAIL_PRS:-0}" = 1 ] && { echo "network error" >&2; exit 1; } ;;
esac
case "$*" in
  *"repo view"*)
    printf '%s\n' '{"nameWithOwner":"owner/repo"}'
    ;;
  *"issue view"*foam-only*)
    echo "should not gh issue view a foam-only slug" >&2
    exit 2
    ;;
  *"issue view"*11*)
    printf '%s\n' '{"labels":[{"name":"triage:ready-for-human"}]}'
    ;;
  *"issue view"*)
    printf '%s\n' '{"labels":[{"name":"type:fix"}]}'
    ;;
  *"pr list"*feat/foam-only-*)
    if [ "${GH_STUB_FOAM_PR:-0}" = 1 ]; then echo '[{"number":88}]'; else echo '[]'; fi
    ;;
  *"pr list"*feat/12-*)
    echo '[{"number":99}]'
    ;;
  *"pr list"*)
    echo '[]'
    ;;
  *)
    exit 0
    ;;
esac
EOF
chmod +x "$STUB_BIN/gh"

export PATH="$STUB_BIN:$PATH"
export ORCH_FOAM_BOARD="$STUB_BIN/fake-gh-board.sh"

echo "# orch-auto refuses to run without ORCH_AGENT (no parent session to inherit)"
set +e
out_na="$(bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>&1)"
rc_na=$?
set -e
assert_eq "$rc_na" "2" "no ORCH_AGENT -> exit 2"
assert_contains "$out_na" "ORCH_AGENT is not set" "names the missing env"
export ORCH_AGENT=claude

echo "# orch-auto --dry-run filters human + open-PR"
out="$(bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>"$TMP/stderr")"
assert_eq "$out" "10" "only eligible #10 printed"
assert_contains "$(cat "$TMP/stderr")" "skip #11" "logs skip human"
assert_contains "$(cat "$TMP/stderr")" "skip #12" "logs skip open PR"

echo "# orch-auto fails closed when gh lookups fail (rate limit / network)"
out_fl="$(GH_STUB_FAIL_LABELS=1 bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>"$TMP/stderr")"
assert_eq "$out_fl" "" "labels lookup failure -> nothing dispatched"
assert_contains "$(cat "$TMP/stderr")" "skip #11 (labels lookup failed; fail closed)" "logs labels failure skip"
assert_contains "$(cat "$TMP/stderr")" "no eligible Ready issues" "no eligible when all lookups fail"
out_fp="$(GH_STUB_FAIL_PRS=1 bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>"$TMP/stderr")"
assert_eq "$out_fp" "" "open-PR lookup failure -> nothing dispatched"
assert_contains "$(cat "$TMP/stderr")" "skip #10 (open-PR lookup failed; fail closed)" "logs PR failure skip"
assert_contains "$(cat "$TMP/stderr")" "skip #11 (ready-for-human)" "labels still checked first"

echo "# orch-auto foam-only slug skips labels but still skips on open PR"
cat >"$STUB_BIN/fake-foam-board.sh" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
  list-ready) printf '10\nfoam-only\n' ;;
  *) echo "unexpected: $*" >&2; exit 1 ;;
esac
EOF
chmod +x "$STUB_BIN/fake-foam-board.sh"
out_slug="$(ORCH_FOAM_BOARD="$STUB_BIN/fake-foam-board.sh" bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>"$TMP/stderr")"
assert_eq "$out_slug" $'10\nfoam-only' "foam-only eligible when no open PR (labels skipped)"
! grep -q 'skip #foam-only' "$TMP/stderr" || { FAILS=$((FAILS + 1)); echo "FAIL foam-only skipped without open PR" >&2; }
out_slug_pr="$(ORCH_FOAM_BOARD="$STUB_BIN/fake-foam-board.sh" GH_STUB_FOAM_PR=1 bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>"$TMP/stderr")"
assert_eq "$out_slug_pr" "10" "foam-only skipped when open PR on feat/foam-only-issue-foam-only"
assert_contains "$(cat "$TMP/stderr")" "skip #foam-only" "logs skip foam-only open PR"

echo "# orch-auto dry-run uses real foam-board fixture (ready listed, backlog/blocked not)"
FIX="$(cd "$SCRIPTS/../../cda-kanban/scripts/tests/fixtures/foam-board" && pwd)"
FOAM_SH="$(cd "$SCRIPTS/../../cda-kanban/scripts" && pwd)/foam-board.sh"
mkdir -p "$TMP/foamfx"
cp -R "$FIX/." "$TMP/foamfx"
git -C "$TMP/foamfx" init -q
git -C "$TMP/foamfx" config user.email auto@test
git -C "$TMP/foamfx" config user.name auto
git -C "$TMP/foamfx" add foam
git -C "$TMP/foamfx" commit -qm board
out_fx="$(ORCH_FOAM_BOARD="$FOAM_SH" bash "$SCRIPTS/orch-auto.sh" --repo "$TMP/foamfx" --dry-run 2>"$TMP/stderr")"
assert_eq "$out_fx" "10" "fixture: only github_issue 10 (backlog + blocked skipped)"
if grep -qi 'project' "$TMP/stderr"; then
  FAILS=$((FAILS + 1)); echo "FAIL fixture orch-auto mentioned Projects" >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# orch-auto keeps only the newest 50 logs"
for n in $(seq -w 1 60); do
  : >"$TMP/.orch/auto/old-$n.log"
  touch -t "$(printf '2000010112%02d' $((10#$n - 1)))" "$TMP/.orch/auto/old-$n.log"  # minutes 00-59: valid for GNU and BSD touch
done
bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run >/dev/null 2>&1
# 49 kept + the new log; 49 total when the new log shares a second-resolution name with an earlier one
n_logs="$(find "$TMP/.orch/auto" -name '*.log' | wc -l | tr -d ' ')"
[ "$n_logs" -le 50 ] && [ "$n_logs" -ge 49 ] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL log count capped at 50 (got $n_logs)" >&2; }
assert_file "$TMP/.orch/auto/old-60.log" "newest old log kept"
[ ! -e "$TMP/.orch/auto/old-01.log" ] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL oldest log not pruned" >&2; }

echo "# orch-auto second run exits 75 (EX_TEMPFAIL) while the lock is held"
mkdir -p "$TMP/.orch"
(
  # Hold the lock the way orch_acquire_lock takes it on this host (flock, else mkdir + live pid).
  if command -v flock >/dev/null 2>&1; then
    exec 9>"$TMP/.orch/auto.lock"
    flock -n 9
  else
    mkdir "$TMP/.orch/auto.lock.d"
    echo "$$" >"$TMP/.orch/auto.lock.d/pid"
  fi
  set +e
  out2="$(bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>&1)"
  rc2=$?
  set -e
  assert_eq "$rc2" "75" "second run exits 75 when lock held"
  assert_contains "$out2" "another run holds" "reports lock contention"
  # Counters do not leave the subshell: report failures through its status.
  [ "$FAILS" -eq 0 ]
) || FAILS=$((FAILS + 1))

summary
