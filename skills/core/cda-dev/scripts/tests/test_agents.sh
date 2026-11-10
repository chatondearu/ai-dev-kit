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
cat >"$FAKE/custom-stub" <<EOF
#!$STUB_BASH
echo "custom-stub-marker \$*"; exit 0
EOF
chmod +x "$FAKE/custom-stub"

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

PATH="$STUB_PATH" ORCH_AGENT= ORCH_AGENT_BIN=
orch_detect_agent >/dev/null
assert_eq "${ORCH_AGENT_BIN:-}" "claude" "BIN set before ORCH_AGENT override"
export ORCH_AGENT="$FAKE/custom-stub"
out_override="$(PATH="$STUB_PATH" orch_agent_run "$FAKE" "$PROMPT" 2>&1)"
assert_contains "$out_override" "custom-stub-marker" "orch_agent_run honors ORCH_AGENT when BIN stale"

# Override
export ORCH_AGENT="$FAKE/claude --print"
kind2="$(orch_detect_agent)"
assert_eq "$kind2" "custom" "custom override"

# Resolution order per role (ADR-010)
unset ORCH_AGENT
TAB=$'\t'
FAKE2="$(mktemp -d)"
trap 'PATH="$_ORIG_PATH"; rm -rf "$FAKE" "$FAKE2"' EXIT
for c in cursor-agent claude codex; do
  printf '#!%s\necho "%s-ok $*"; cat\n' "$STUB_BASH" "$c" >"$FAKE2/$c"
  chmod +x "$FAKE2/$c"
done
P2="$FAKE2:$STUB_PATH"

r="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_VERIFIER=codex orch_resolve_agent verifier)"
assert_eq "$r" "codex${TAB}role-override" "role override beats ORCH_AGENT"
r="$(PATH="$P2" ORCH_AGENT_PR_FIX=codex orch_resolve_agent pr-fix)"
assert_eq "$r" "codex${TAB}role-override" "pr-fix maps to ORCH_AGENT_PR_FIX"
r="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_VERIFIER=codex orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}override" "global override for other roles"
r="$(PATH="$P2" CLAUDECODE=1 orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}parent" "CLAUDECODE=1 beats cursor-agent on PATH"
r="$(PATH="$P2" CODEX_THREAD_ID=t1 orch_resolve_agent worker)"
assert_eq "$r" "codex${TAB}parent" "CODEX_THREAD_ID -> codex"
r="$(PATH="$FAKE" CLAUDECODE=1 CURSOR_INVOKED_AS=agent orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}parent" "env marker order: claude first"
r="$(PATH="$P2" orch_resolve_agent worker)"
assert_eq "$r" "cursor-agent${TAB}path" "no marker, no ancestor -> PATH order"
mkdir "$FAKE2/only"; cp "$FAKE2/cursor-agent" "$FAKE2/only/"
r="$(PATH="$FAKE2/only" CLAUDECODE=1 orch_resolve_agent worker)"
assert_eq "$r" "cursor-agent${TAB}path" "parent CLI not installed -> PATH"

# Fake ancestry: <start> -> 500 bash -> 400 node (args: cursor-agent script)
cat >"$FAKE2/fake-ps" <<STUB
#!$STUB_BASH
pid="\${*: -1}"
case "\$*:\$pid" in
  *comm=*:400) echo "1 node" ;;
  *comm=*:500) echo "400 zsh" ;;
  *comm=*) echo "500 bash" ;;
  *args=*:400) echo "node /opt/bin/cursor-agent -p" ;;
  *args=*) echo "zsh -c orch.sh" ;;
esac
STUB
chmod +x "$FAKE2/fake-ps"
r="$(PATH="$P2" ORCH_PS="$FAKE2/fake-ps" orch_resolve_agent worker)"
assert_eq "$r" "cursor-agent${TAB}parent" "ps ancestry -> cursor-agent"
r="$(PATH="$P2" ORCH_PS="$FAKE2/fake-ps" CLAUDECODE=1 orch_resolve_agent worker)"
assert_eq "$r" "claude${TAB}parent" "env marker beats ps ancestry"

# orch_agent_run picks the CLI of ORCH_ROLE; codex runs `exec -` on stdin
out_role="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_REVIEWER="$FAKE/custom-stub" ORCH_ROLE=reviewer orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_role" "custom-stub-marker" "orch_agent_run honors ORCH_ROLE"
out_codex="$(PATH="$P2" ORCH_AGENT=codex orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_codex" "codex-ok exec --dangerously-bypass-approvals-and-sandbox -" "codex exec flags"
assert_contains "$out_codex" "hi" "codex reads the prompt on stdin"

out_dry="$(PATH="$P2" ORCH_AGENT_VERIFIER=codex CLAUDECODE=1 orch_print_agents)"
assert_contains "$out_dry" "agent[worker]: claude (parent)" "dry-run line worker"
assert_contains "$out_dry" "agent[verifier]: codex (role-override)" "dry-run line verifier"

summary
