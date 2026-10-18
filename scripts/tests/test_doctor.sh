#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FAILS=0
assert_eq() {
  if [ "$1" != "$2" ]; then
    echo "FAIL: $3 (got '$1' want '$2')" >&2
    FAILS=$((FAILS + 1))
  else
    echo "ok: $3"
  fi
}
assert_contains() {
  if ! printf '%s' "$1" | grep -q "$2"; then
    echo "FAIL: $3 (missing '$2')" >&2
    FAILS=$((FAILS + 1))
  else
    echo "ok: $3"
  fi
}

FAKE="$(mktemp -d)"
_ORIG_PATH="$PATH"
STUB_BASH="$(command -v bash)"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE"' EXIT

# Empty PATH except stub git — expect FAIL (no agent)
cat >"$FAKE/git" <<EOF
#!$STUB_BASH
echo "git version stub"
EOF
chmod +x "$FAKE/git"

# Keep coreutils; exclude real agent CLIs by putting FAKE first and omitting their dirs
stub_path() {
  local clean="$FAKE" d
  local IFS=:
  for d in /usr/bin /bin $_ORIG_PATH; do
    [ -z "$d" ] && continue
    if [ -x "$d/agent" ] || [ -x "$d/cursor-agent" ] || [ -x "$d/claude" ] || [ -x "$d/opencode" ]; then
      continue
    fi
    clean="${clean}:$d"
  done
  printf '%s\n' "$clean"
}
DPATH="$(stub_path)"

set +e
out="$(PATH="$DPATH" HOME="$FAKE/home" "$STUB_BASH" "$ROOT/scripts/doctor.sh" 2>&1)"
rc=$?
set -e
assert_eq "$rc" "1" "doctor fails without agent CLI"
assert_contains "$out" "no agent CLI" "mentions missing agent"

cat >"$FAKE/claude" <<EOF
#!$STUB_BASH
true
EOF
chmod +x "$FAKE/claude"
mkdir -p "$FAKE/home/.cursor/skills/cda-dev"
DPATH="$(stub_path)"

set +e
out2="$(PATH="$DPATH" HOME="$FAKE/home" CURSOR_HOME="$FAKE/home/.cursor" "$STUB_BASH" "$ROOT/scripts/doctor.sh" 2>&1)"
rc2=$?
set -e
assert_eq "$rc2" "0" "doctor OK with claude stub"
assert_contains "$out2" "ok   claude" "reports claude"
assert_contains "$out2" "recommended tools" "points at recommended.md"

if [ "$FAILS" -gt 0 ]; then
  echo "FAILS=$FAILS" >&2
  exit 1
fi
echo "summary: all ok"
