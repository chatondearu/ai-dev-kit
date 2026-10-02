#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/loop.sh"

TMP="$(mktemp -d)"
FAKE="$(mktemp -d)"
PROMPT="$(mktemp)"
echo "fix" >"$PROMPT"
_ORIG_PATH="$PATH"
STUB_BASH="$(command -v bash)"

cleanup() {
  PATH="$_ORIG_PATH"
  rm -rf "$TMP" "$FAKE" "$PROMPT"
}
trap cleanup EXIT

# Test 1: Agent fails verify once then succeeds on iter 2
cat >"$FAKE/claude" <<EOF
#!$STUB_BASH
if [ -f step1 ]; then
  touch ok
else
  touch step1
fi
exit 0
EOF
chmod +x "$FAKE/claude"

cat >"$FAKE/npm" <<EOF
#!$STUB_BASH
test -f ok
EOF
chmod +x "$FAKE/npm"

printf '{"scripts":{"test":"npm test"}}\n' >"$TMP/package.json"

export PATH="$FAKE:$_ORIG_PATH" ORCH_AGENT= ORCH_AGENT_BIN=claude ORCH_MAX_ITER=3

set +e
out="$(orch_loop_worker "$TMP" "$PROMPT" 2>&1)"
rc=$?
set -e

assert_eq "$rc" "0" "loop succeeds when verify eventually passes"
assert_file "$TMP/ok" "verify passing artifact created"
assert_contains "$out" "orch loop iter 1" "logs iter 1"
assert_contains "$out" "orch loop iter 2" "logs iter 2"
assert_contains "$out" "verify ok" "logs verify ok"

# Test 2: Verify never succeeds -> exhausts ORCH_MAX_ITER and returns 1 with 'blocked'
TMP2="$(mktemp -d)"
printf '{"scripts":{"test":"npm test"}}\n' >"$TMP2/package.json"

cat >"$FAKE/npm" <<EOF
#!$STUB_BASH
exit 1
EOF

set +e
out2="$(ORCH_MAX_ITER=2 orch_loop_worker "$TMP2" "$PROMPT" 2>&1)"
rc2=$?
set -e
rm -rf "$TMP2"

assert_eq "$rc2" "1" "loop fails when max iter exhausted"
assert_contains "$out2" "blocked" "logs blocked when exhausted"
assert_contains "$out2" "orch loop iter 2/2" "reaches max iteration"

summary
