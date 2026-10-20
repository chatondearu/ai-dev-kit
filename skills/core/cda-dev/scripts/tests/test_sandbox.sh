#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/sandbox.sh
source "$TROOT/../lib/sandbox.sh"

ORCH_SANDBOX=0 ORCH_WT=/tmp
out="$(orch_sandbox_wrap bash -c 'echo ok')"
assert_eq "$out" "ok" "passthrough"

ORCH_SANDBOX=0 ORCH_WT=/tmp ORCH_PROMPTS_DIR=/tmp
out_dir="$(orch_sandbox_wrap bash -c 'echo dir-ok')"
assert_eq "$out_dir" "dir-ok" "passthrough with ORCH_PROMPTS_DIR set"

# Missing bwrap + sandbox requested → hard fail (do not run unsandboxed)
FAKE="$(mktemp -d)"
_ORIG_PATH="$PATH"
# Empty PATH so bwrap is not found; keep bash via env in command
export PATH="$FAKE"
ORCH_SANDBOX=1 ORCH_WT=/tmp
set +e
orch_sandbox_wrap /bin/echo should-not-run >/dev/null 2>"$FAKE/err"
rc=$?
set -e
PATH="$_ORIG_PATH"
assert_eq "$rc" "1" "sandbox without bwrap exits 1"
if grep -q 'refusing to continue unsandboxed' "$FAKE/err"; then
  PASSES=$((PASSES + 1))
  printf 'ok   sandbox error message\n'
else
  FAILS=$((FAILS + 1))
  printf 'FAIL expected refusal message\n' >&2
fi
rm -rf "$FAKE"

summary
