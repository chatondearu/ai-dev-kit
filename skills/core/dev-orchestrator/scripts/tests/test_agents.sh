#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"

# Fake PATH with only a stub "claude"
FAKE="$(mktemp -d)"
_ORIG_PATH="$PATH"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE"' EXIT
STUB_BASH="$(command -v bash)"
cat >"$FAKE/claude" <<EOF
#!$STUB_BASH
echo "claude-ok \$*"; exit 0
EOF
chmod +x "$FAKE/claude"

# PATH with stub claude first; drop dirs that expose real agent/cursor-agent CLIs
orch_test_stub_path() {
  local fake="$1" clean d
  clean="$fake"
  local IFS=:
  for d in $PATH; do
    [ -z "$d" ] && continue
    if [ -x "$d/agent" ] || [ -x "$d/cursor-agent" ]; then
      continue
    fi
    clean="${clean}:$d"
  done
  printf '%s\n' "$clean"
}
STUB_PATH="$(orch_test_stub_path "$FAKE")"
PATH="$STUB_PATH" ORCH_AGENT= ORCH_AGENT_BIN=
kind="$(orch_detect_agent)"
assert_eq "$kind" "claude" "detect claude"
PATH="$STUB_PATH" ORCH_AGENT= ORCH_AGENT_BIN=
orch_detect_agent >/dev/null
assert_eq "${ORCH_AGENT_BIN:-}" "claude" "ORCH_AGENT_BIN set"

PROMPT="$FAKE/prompt.txt"
echo "hi" >"$PROMPT"
out="$(PATH="$STUB_PATH" orch_agent_run "$FAKE" "$PROMPT" 2>&1 || true)"
assert_contains "$out" "claude-ok" "invoke stub"

# Override
export ORCH_AGENT="$FAKE/claude --print"
kind2="$(orch_detect_agent)"
assert_eq "$kind2" "custom" "custom override"

summary
