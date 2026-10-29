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

if [ "$FAILS" -gt 0 ]; then
  echo "FAILS=$FAILS" >&2
  exit 1
fi
echo "summary: all ok"
