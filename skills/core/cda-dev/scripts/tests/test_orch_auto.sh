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

if ! command -v flock >/dev/null 2>&1; then
  echo "SKIP test_orch_auto.sh (flock not installed)"
  exit 0
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

echo "# orch-auto --dry-run --once filters human + open-PR"
out="$(bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --once --dry-run 2>"$TMP/stderr")"
assert_eq "$out" "10" "only eligible #10 printed"
assert_contains "$(cat "$TMP/stderr")" "skip #11" "logs skip human"
assert_contains "$(cat "$TMP/stderr")" "skip #12" "logs skip open PR"

echo "# orch-auto flock second run exits 0"
mkdir -p "$TMP/.orch"
(
  exec 9>"$TMP/.orch/auto.lock"
  flock -n 9
  set +e
  out2="$(bash "$SCRIPTS/orch-auto.sh" --repo "$TMP" --once --dry-run 2>&1)"
  rc2=$?
  set -e
  assert_eq "$rc2" "0" "second run exits 0 when lock held"
  assert_contains "$out2" "another run holds" "reports lock contention"
)

summary
