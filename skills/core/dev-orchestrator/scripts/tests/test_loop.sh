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

# Test 3: Prompt substitution helper
TMP_TPL="$(mktemp)"
TMP_DEST="$(mktemp -u)"
printf 'Issue: {{ISSUE}}, WT: {{WORKTREE}}, Unknown: {{UNKNOWN_KEY}}\n' >"$TMP_TPL"
orch_render_prompt "$TMP_TPL" "$TMP_DEST" "ISSUE=99" "WORKTREE=/tmp/wt-99"
dest_content="$(cat "$TMP_DEST")"
rm -f "$TMP_TPL" "$TMP_DEST"
assert_contains "$dest_content" "Issue: 99" "prompt helper substitutes known key"
assert_contains "$dest_content" "WT: /tmp/wt-99" "prompt helper substitutes second key"
assert_contains "$dest_content" "Unknown: N/A" "prompt helper converts unknown key to N/A"

# Test 4: When verify is fallback 'true' and agent fails -> blocked (not silently passed)
TMP_EMPTY="$(mktemp -d)"
cat >"$FAKE/claude-fail" <<EOF
#!$STUB_BASH
exit 1
EOF
chmod +x "$FAKE/claude-fail"

set +e
out_fail="$(ORCH_AGENT="$FAKE/claude-fail" ORCH_MAX_ITER=2 orch_loop_worker "$TMP_EMPTY" "$PROMPT" 2>&1)"
rc_fail=$?
set -e
rm -rf "$TMP_EMPTY"

assert_eq "$rc_fail" "1" "loop fails when verify is true and agent fails"
assert_contains "$out_fail" "blocked" "logs blocked when agent fails on true verify"
assert_contains "$out_fail" "agent failed and verify is fallback true" "logs fallback true agent failure"

# Test 5: When verify is fallback 'true' and agent succeeds (exit 0) -> loop succeeds
TMP_EMPTY2="$(mktemp -d)"
cat >"$FAKE/claude-pass" <<EOF
#!$STUB_BASH
exit 0
EOF
chmod +x "$FAKE/claude-pass"

set +e
out_pass="$(ORCH_AGENT="$FAKE/claude-pass" ORCH_MAX_ITER=2 orch_loop_worker "$TMP_EMPTY2" "$PROMPT" 2>&1)"
rc_pass=$?
set -e
rm -rf "$TMP_EMPTY2"

assert_eq "$rc_pass" "0" "loop succeeds when verify is true and agent exits 0"
assert_contains "$out_pass" "verify ok" "logs verify ok on agent exit 0"

# Test 6: When verify is fallback 'true', agent exits 1, but sentinel .orch/agent-ok exists -> succeeds
TMP_EMPTY3="$(mktemp -d)"
cat >"$FAKE/claude-sentinel" <<EOF
#!$STUB_BASH
mkdir -p .orch
touch .orch/agent-ok
exit 1
EOF
chmod +x "$FAKE/claude-sentinel"

set +e
out_sentinel="$(ORCH_AGENT="$FAKE/claude-sentinel" ORCH_MAX_ITER=2 orch_loop_worker "$TMP_EMPTY3" "$PROMPT" 2>&1)"
rc_sentinel=$?
set -e
rm -rf "$TMP_EMPTY3"

assert_eq "$rc_sentinel" "0" "loop succeeds when agent exits 1 but .orch/agent-ok sentinel exists"
assert_contains "$out_sentinel" "verify ok" "logs verify ok when sentinel present"

summary
