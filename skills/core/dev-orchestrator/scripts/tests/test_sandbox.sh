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
summary
