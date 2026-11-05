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
assert_contains "$out2" "Host tools" "reports host tools section"
assert_contains "$out2" "jq" "reports jq"
assert_contains "$out2" "flock" "reports flock"
assert_contains "$out2" "graphify" "mentions graphify optional"
assert_contains "$out2" "CDA_GRAPHIFY" "mentions CDA_GRAPHIFY"

# Graphify present + CDA_GRAPHIFY=1 → ok lines
cat >"$FAKE/graphify" <<EOF
#!$STUB_BASH
true
EOF
chmod +x "$FAKE/graphify"
DPATH="$(stub_path)"
set +e
out_gf="$(PATH="$DPATH" HOME="$FAKE/home" CURSOR_HOME="$FAKE/home/.cursor" CDA_GRAPHIFY=1 "$STUB_BASH" "$ROOT/scripts/doctor.sh" 2>&1)"
rc_gf=$?
set -e
assert_eq "$rc_gf" "0" "doctor OK with graphify stub"
assert_contains "$out_gf" "ok   graphify" "reports graphify ok"
assert_contains "$out_gf" "ok   CDA_GRAPHIFY=1" "reports CDA_GRAPHIFY"

# Stock bash < 4 (macOS /bin/bash 3.2): doctor itself must run and warn that orch needs 4+
if [ -x /bin/bash ] && [ "$(/bin/bash -c 'echo ${BASH_VERSINFO[0]}')" -lt 4 ]; then
  out3="$(PATH="$DPATH" HOME="$FAKE/home" CURSOR_HOME="$FAKE/home/.cursor" /bin/bash "$ROOT/scripts/doctor.sh" 2>&1)"
  assert_contains "$out3" "orch.sh needs bash 4+" "warns on bash < 4"
fi

if [ "$FAILS" -gt 0 ]; then
  echo "FAILS=$FAILS" >&2
  exit 1
fi
echo "summary: all ok"
