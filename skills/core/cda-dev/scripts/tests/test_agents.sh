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
assert_contains "$out_codex" "codex-ok exec --dangerously-bypass-approvals-and-sandbox -m gpt-6.1-sol -c model_reasoning_effort=high -" "codex exec flags + medium tier"
assert_contains "$out_codex" "hi" "codex reads the prompt on stdin"

out_dry="$(PATH="$P2" ORCH_AGENT_VERIFIER=codex CLAUDECODE=1 orch_print_agents)"
assert_contains "$out_dry" "agent[worker]: claude (parent)" "dry-run line worker"
assert_contains "$out_dry" "agent[verifier]: codex (role-override)" "dry-run line verifier"

# Tiers (config/tiers.tsv): role defaults, per-role and current-tier env, per-CLI flags.
export ORCH_RUN_ID=t1 ORCH_REPO="$FAKE2"
assert_eq "$(orch_tier_for_role worker)" "medium" "default tier: medium"
assert_eq "$(orch_tier_for_role integrator)" "small" "integrator default: small"
assert_eq "$(ORCH_TIER_PR_FIX=large orch_tier_for_role pr-fix)" "large" "ORCH_TIER_PR_FIX"
assert_eq "$(ORCH_TIER=large ORCH_TIER_WORKER=small orch_tier_for_role worker)" "large" "ORCH_TIER (current) beats role tier"
assert_eq "$(orch_tier_flags agent large)" "--model claude-opus-5-5-high" "agent uses cursor rows"
out_t="$(PATH="$P2" ORCH_AGENT=claude orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_t" "claude-ok -p --dangerously-skip-permissions --model sonnet --effort high" "claude medium flags"
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_ROLE=integrator orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_t" "--model sonnet --effort low" "integrator runs small"
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_TIER_REVIEWER=large ORCH_ROLE=reviewer orch_agent_run "$FAKE" "$PROMPT")"
assert_contains "$out_t" "--model opus --effort high" "ORCH_TIER_REVIEWER=large"
out_t="$(PATH="$P2" ORCH_AGENT=cursor-agent orch_agent_run "$FAKE" "$PROMPT" </dev/null)"
assert_contains "$out_t" "cursor-agent-ok -p --force --model auto hi" "cursor medium flags before prompt"

# --model in the agent command → pinned, no tier flags.
out_t="$(PATH="$P2" ORCH_AGENT="claude --model haiku" orch_agent_run "$FAKE" "$PROMPT")"
assert_eq "${out_t%%$'\n'*}" "claude-ok --model haiku -p --dangerously-skip-permissions" "pinned: no tier flags"
assert_eq "$(orch_agent_tier "codex -m gpt-x" worker)" "pinned${TAB}" "-m pins too"

# ORCH_TIERS_FILE replaces the kit table; a missing one is an error.
printf '# custom\nclaude\tmedium\t--model haiku\n' >"$FAKE2/tiers.tsv"
out_t="$(PATH="$P2" ORCH_AGENT=claude ORCH_TIERS_FILE="$FAKE2/tiers.tsv" orch_agent_run "$FAKE" "$PROMPT")"
assert_eq "${out_t%%$'\n'*}" "claude-ok -p --dangerously-skip-permissions --model haiku" "ORCH_TIERS_FILE row"
rc=0; err="$(PATH="$P2" ORCH_AGENT=claude ORCH_TIERS_FILE="$FAKE2/nope.tsv" orch_agent_run "$FAKE" "$PROMPT" 2>&1)" || rc=$?
assert_eq "$rc" "1" "missing ORCH_TIERS_FILE fails"
assert_contains "$err" "tiers file not readable" "missing ORCH_TIERS_FILE message"

# CLI without a row: no flags, one warning per run.
err="$( { orch_agent_tier "$FAKE/custom-stub" worker; orch_agent_tier "$FAKE/custom-stub" integrator; } 2>&1 >/dev/null)"
assert_eq "$(grep -c 'no tier row for custom-stub' <<<"$err")" "1" "unknown CLI warns once per run"
assert_eq "$(orch_agent_tier "$FAKE/custom-stub" worker 2>/dev/null)" "medium${TAB}" "unknown CLI: no flags"
unset ORCH_RUN_ID

out_dry="$(PATH="$P2" ORCH_AGENT=claude ORCH_AGENT_INTEGRATOR="claude --model haiku" orch_print_agents)"
assert_contains "$out_dry" "tier[worker]: medium --model sonnet --effort high" "dry-run tier line"
assert_contains "$out_dry" "tier[integrator]: pinned" "dry-run pinned line"

# Per-worker json gets cli/model/effort/tier of the worker's agent.
if command -v jq >/dev/null 2>&1; then
  source "$TROOT/../lib/progress.sh"
  export ORCH_PROGRESS=0 ORCH_REPO="$FAKE2"
  PATH="$P2" ORCH_AGENT=claude ORCH_WORKER_ID=5 orch_agent_run "$FAKE" "$PROMPT" >/dev/null
  assert_eq "$(jq -c '[.id, .cli, .model, .effort, .tier]' "$FAKE2/.orch/run/5.json")" \
    '["5","claude","sonnet","high","medium"]' "worker json records the worker tier"
fi

summary
