#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/loop.sh"
source "$TROOT/../lib/graph.sh"
# --no-graph path: the same loop must still run verify and the grounding verifier.
export ORCH_GRAPH=0
# Tests 1-6 exercise verify/retry only; the verifier (default on, ADR-006) has its own tests below.
export ORCH_GRAPH_VERIFY=0

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

# Test 2b: retry prompt carries the failed verify output (ADR-006)
TMP2B="$(mktemp -d)"
printf '{"scripts":{"test":"npm test"}}\n' >"$TMP2B/package.json"
cat >"$FAKE/npm" <<EOF
#!$STUB_BASH
echo "assertion failed: expected 1 got 2"
exit 1
EOF
cat >"$FAKE/claude-record" <<EOF
#!$STUB_BASH
cat >>.orch/seen-prompts.txt
exit 0
EOF
chmod +x "$FAKE/claude-record"
mkdir -p "$TMP2B/.orch"
set +e
ORCH_AGENT="$FAKE/claude-record" ORCH_MAX_ITER=2 orch_loop_worker "$TMP2B" "$PROMPT" >/dev/null 2>&1
set -e
retry="$TMP2B/.orch/worker-prompt-retry.md"
assert_file "$retry" "retry prompt written after failed verify"
assert_contains "$(cat "$retry")" "## Previous verify output (failed)" "retry prompt has verify section"
assert_contains "$(cat "$retry")" "assertion failed: expected 1 got 2" "retry prompt carries verify output"
assert_contains "$(head -n 1 "$retry")" "fix" "retry prompt keeps the original prompt"
assert_contains "$(cat "$TMP2B/.orch/seen-prompts.txt")" "assertion failed" "iteration 2 agent received the verify output"
rm -rf "$TMP2B"

# Test 2c: no verify command and no allow-no-verify -> rc 3, blocked (fail closed)
TMP2C="$(mktemp -d)"
set +e
out_nov="$(ORCH_AGENT="$FAKE/claude" ORCH_MAX_ITER=2 orch_loop_worker "$TMP2C" "$PROMPT" 2>&1)"
rc_nov=$?
set -e
rm -rf "$TMP2C"
assert_eq "$rc_nov" "3" "loop returns 3 when no verify command"
assert_contains "$out_nov" "verify: no command (add 'orch: allow-no-verify'" "logs no-verify reason"

# Test 2d: stale verifier-ok from a previous pass must not pass a failing verifier (ADR-006)
TMP2D="$(mktemp -d)"
printf '{"scripts":{"test":"npm test"}}\n' >"$TMP2D/package.json"
mkdir -p "$TMP2D/.orch" "$TMP2D/prompts"
touch "$TMP2D/.orch/verifier-ok"
printf '# Verifier (grounding node)\n' >"$TMP2D/prompts/verifier.md"
cat >"$FAKE/npm" <<EOF
#!$STUB_BASH
exit 0
EOF
cat >"$FAKE/claude-verifier-fail" <<EOF
#!$STUB_BASH
case "\$(cat)" in *"Verifier (grounding node)"*) echo "- not grounded" >.orch/verifier-fail ;; esac
exit 0
EOF
chmod +x "$FAKE/claude-verifier-fail"
set +e
out_stale="$(ORCH_GRAPH_VERIFY=1 ORCH_PROMPTS_DIR="$TMP2D/prompts" ORCH_REPO="$TMP2D" ORCH_AGENT="$FAKE/claude-verifier-fail" \
  ORCH_MAX_ITER=1 orch_loop_worker "$TMP2D" "$PROMPT" 2>&1)"
rc_stale=$?
set -e
assert_eq "$rc_stale" "1" "stale verifier-ok + verifier-fail -> loop fails (also with ORCH_GRAPH=0)"
assert_contains "$out_stale" "grounding verifier failed" "logs grounding verifier failure"
rm -rf "$TMP2D"

# Test 2e: verifier failure -> next iteration's prompt carries .orch/verifier-fail
TMP2E="$(mktemp -d)"
printf '{"scripts":{"test":"npm test"}}\n' >"$TMP2E/package.json"
mkdir -p "$TMP2E/.orch" "$TMP2E/prompts"
printf '# Verifier (grounding node)\n' >"$TMP2E/prompts/verifier.md"
cat >"$FAKE/claude-verifier-reason" <<EOF
#!$STUB_BASH
p="\$(cat)"
echo "\$p" >>.orch/seen-prompts.txt
case "\$p" in *"Verifier (grounding node)"*) echo "- plan item 3 not implemented" >.orch/verifier-fail ;; esac
exit 0
EOF
chmod +x "$FAKE/claude-verifier-reason"
set +e
ORCH_GRAPH_VERIFY=1 ORCH_PROMPTS_DIR="$TMP2E/prompts" ORCH_REPO="$TMP2E" ORCH_AGENT="$FAKE/claude-verifier-reason" \
  ORCH_MAX_ITER=2 orch_loop_worker "$TMP2E" "$PROMPT" >/dev/null 2>&1
set -e
retry="$TMP2E/.orch/worker-prompt-retry.md"
assert_file "$retry" "retry prompt written after failed verifier"
assert_contains "$(cat "$retry")" "plan item 3 not implemented" "retry prompt carries verifier reasons"
assert_contains "$(head -n 1 "$retry")" "fix" "verifier retry keeps the original prompt"
assert_contains "$(cat "$TMP2E/.orch/seen-prompts.txt")" "## Previous verifier findings" "iteration 2 agent received the verifier reasons"
rm -rf "$TMP2E"

# Test 2f: missing prompts/verifier.md fails closed (never a silent pass)
TMP2F="$(mktemp -d)"
printf '{"scripts":{"test":"npm test"}}\n' >"$TMP2F/package.json"
set +e
out_nov="$(ORCH_GRAPH_VERIFY=1 ORCH_PROMPTS_DIR="$TMP2F/nope" ORCH_REPO="$TMP2F" ORCH_AGENT="$FAKE/claude" \
  ORCH_MAX_ITER=1 orch_loop_worker "$TMP2F" "$PROMPT" 2>&1)"
rc_nov=$?
set -e
rm -rf "$TMP2F"
assert_eq "$rc_nov" "1" "missing verifier prompt -> loop fails"
assert_contains "$out_nov" "verifier prompt missing" "logs the missing verifier prompt"

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

# Tests 4-6 opt into verify 'true' via ORCH_ALLOW_NO_VERIFY=1 (issue body `orch: allow-no-verify`).
export ORCH_ALLOW_NO_VERIFY=1

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
