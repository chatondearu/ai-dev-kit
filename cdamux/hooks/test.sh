#!/usr/bin/env bash
# cdamux Claude sub-agent hook: sample payloads -> ledger lines. A stub `tmux` on
# PATH answers the session name, so no tmux server is touched.
set -euo pipefail
cd "$(dirname "$0")"
H="$PWD/claude-subagent.js"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
printf '#!/bin/sh\necho "fleet/demo"\n' > "$tmp/bin/tmux"; chmod +x "$tmp/bin/tmux"
export PATH="$tmp/bin:$PATH" HOME="$tmp/home" XDG_STATE_HOME="$tmp/state" TMUX="/tmp/fake,1,0" TMUX_PANE="%1"
L="$tmp/state/cdamux/agents/fleet_demo.jsonl"   # `/` in the session name -> `_`

fail() { echo "FAIL: $*" >&2; exit 1; }
hook() { printf '%s' "$1" | node "$H" || fail "hook exited non-zero"; }

hook '{"session_id":"s1","transcript_path":"/p/s1.jsonl","hook_event_name":"SubagentStart","agent_id":"agent-abc","agent_type":"Explore"}'
hook '{"session_id":"s1","transcript_path":"/p/s1.jsonl","hook_event_name":"SubagentStop","agent_id":"abc","agent_type":"Explore","agent_transcript_path":"~/x/agent-abc.jsonl","last_assistant_message":"done"}'
[ "$(wc -l < "$L" | tr -d ' ')" = 2 ] || fail "expected 2 ledger lines"
jq -e 'select(.event=="start") | .cli=="claude" and .id=="abc" and .parent_id=="s1" and .name=="Explore"
  and .transcript=="/p/s1/subagents/agent-abc.jsonl" and (.ts|type)=="number"' "$L" >/dev/null || fail "start line wrong"
jq -e --arg t "$HOME/x/agent-abc.jsonl" 'select(.event=="stop") | .id=="abc" and .transcript==$t' "$L" >/dev/null || fail "stop line wrong"

# skipped: internal agent (empty agent_type), other events, bad JSON, outside tmux
hook '{"hook_event_name":"SubagentStop","agent_id":"x","agent_type":""}'
hook '{"hook_event_name":"Stop","agent_id":"x","agent_type":"Explore"}'
hook 'not json'
printf '%s' '{"hook_event_name":"SubagentStart","agent_id":"y","agent_type":"Explore"}' | env -u TMUX node "$H"
[ "$(wc -l < "$L" | tr -d ' ')" = 2 ] || fail "skipped payloads wrote a line"

# trimming keeps the last 500 lines
for i in $(seq 1 520); do printf '{"ts":0,"cli":"claude","event":"start","id":"old%s"}\n' "$i"; done >> "$L"
hook '{"session_id":"s1","hook_event_name":"SubagentStart","agent_id":"last","agent_type":"Plan"}'
[ "$(wc -l < "$L" | tr -d ' ')" = 500 ] || fail "ledger not trimmed to 500"
[ "$(tail -n 1 "$L" | jq -r .id)" = last ] || fail "trim lost the newest line"

echo "ok: cdamux claude sub-agent hook"
