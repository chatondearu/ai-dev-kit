#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SCRIPT="$ROOT/skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh"
FAILS=0

assert_rc() {
  local want="$1"
  shift
  set +e
  "$@"
  local got=$?
  set -e
  if [ "$got" != "$want" ]; then
    echo "FAIL: expected exit $want got $got: $*" >&2
    FAILS=$((FAILS + 1))
  else
    echo "ok: $*"
  fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/good/foam/product" "$TMP/good/prd" "$TMP/good/plans"
printf '# index\n' >"$TMP/good/foam/index.md"
printf '# layout\nActive: plans/; archived: foam/archive/plans/\n' >"$TMP/good/foam/product/documentation-layout.md"

mkdir -p "$TMP/bad/foam/product" "$TMP/bad/docs/superpowers/specs"
printf '# index\n' >"$TMP/bad/foam/index.md"
printf '# oops\n' >"$TMP/bad/docs/superpowers/specs/x.md"

chmod +x "$SCRIPT"
bash -n "$SCRIPT"

assert_rc 0 bash "$SCRIPT" --check "$TMP/good"
assert_rc 1 bash "$SCRIPT" --check "$TMP/bad"

# --archive-done with a stub gh: closed plan moves and its links are rewritten (BSD sed safe).
A="$TMP/arch"
mkdir -p "$A/bin" "$A/repo/plans" "$A/repo/prd" "$A/repo/foam"
printf '#!/usr/bin/env bash\n[ "$1 $2" = "auth status" ] && exit 0\necho CLOSED\n' >"$A/bin/gh"
chmod +x "$A/bin/gh"
printf '# PLAN-5\n[[../prd/PRD-5-x|PRD-5]] [[../foam/index|i]] [p](../prd/PRD-5-x.md)\n' >"$A/repo/plans/PLAN-5-x.md"
git -C "$A/repo" init -q
git -C "$A/repo" remote add origin https://github.com/o/r.git
git -C "$A/repo" add -A
PATH="$A/bin:$PATH" assert_rc 0 bash "$SCRIPT" --archive-done "$A/repo"
ARCHIVED="$A/repo/foam/archive/plans/PLAN-5-x.md"
assert_rc 0 grep -qF '[[../../prd/PRD-5-x|PRD-5]] [[../../foam/index|i]] [p](../../prd/PRD-5-x.md)' "$ARCHIVED"
assert_rc 1 test -e "$ARCHIVED.bak"

if [ "$FAILS" -gt 0 ]; then
  echo "FAILS=$FAILS" >&2
  exit 1
fi
echo "summary: all ok"
