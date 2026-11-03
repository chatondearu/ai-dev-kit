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

# No flock on the host (macOS): stub it so the dry-run filters still run; the
# lock-contention case below needs the real one and is skipped.
HAVE_FLOCK=1
if ! command -v flock >/dev/null 2>&1; then
  HAVE_FLOCK=0
  printf '#!/usr/bin/env bash\nexit 0\n' >"$STUB_BIN/flock"
  chmod +x "$STUB_BIN/flock"
fi
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
  *"issue view"*11*)
    printf '%s\n' '{"labels":[{"name":"triage:ready-for-human"}]}'
    ;;
  *"issue view"*)
    printf '%s\n' '{"labels":[{"name":"type:fix"}]}'
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
export ORCH_GH_BOARD="$STUB_BIN/fake-gh-board.sh"

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

echo "# orch-auto keeps only the newest 50 logs"
for n in $(seq -w 1 60); do
  : >"$TMP/.orch/auto/old-$n.log"
  touch -t "20000101${n}00" "$TMP/.orch/auto/old-$n.log"
done
bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run >/dev/null 2>&1
# 49 kept + the new log; 49 total when the new log shares a second-resolution name with an earlier one
n_logs="$(find "$TMP/.orch/auto" -name '*.log' | wc -l | tr -d ' ')"
[ "$n_logs" -le 50 ] && [ "$n_logs" -ge 49 ] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL log count capped at 50 (got $n_logs)" >&2; }
assert_file "$TMP/.orch/auto/old-60.log" "newest old log kept"
[ ! -e "$TMP/.orch/auto/old-01.log" ] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL oldest log not pruned" >&2; }

if [ "$HAVE_FLOCK" = 0 ]; then
  echo "SKIP flock contention case (flock not installed)"
  summary
  exit $?
fi

echo "# orch-auto flock second run exits 75 (EX_TEMPFAIL)"
mkdir -p "$TMP/.orch"
(
  exec 9>"$TMP/.orch/auto.lock"
  flock -n 9
  set +e
  out2="$(bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --dry-run 2>&1)"
  rc2=$?
  set -e
  assert_eq "$rc2" "75" "second run exits 75 when lock held"
  assert_contains "$out2" "another run holds" "reports lock contention"
)

summary
