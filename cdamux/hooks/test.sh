#!/usr/bin/env bash
# cdamux sub-agent adapters (Claude, Cursor, opencode): sample payloads -> ledger lines. A stub `tmux` on
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

# Cursor: payloads shaped from https://cursor.com/docs/hooks (not recorded live).
C="$PWD/cursor-subagent.js"
cursor() { printf '%s' "$1" | node "$C" || fail "cursor hook exited non-zero"; }
rm -f "$L"
out="$(cursor '{"hook_event_name":"subagentStart","transcript_path":"/c/agent-transcripts/p1/p1.jsonl","subagent_id":"u-1","subagent_type":"explore","parent_conversation_id":"p1","task":"x"}')"
[ "$out" = '{"permission":"allow"}' ] || fail "cursor start must answer allow, got: $out"
out="$(cursor '{"hook_event_name":"subagentStop","subagent_type":"explore","status":"error","agent_transcript_path":"/c/agent-transcripts/p1/subagents/u-1.jsonl"}')"
[ "$out" = '{}' ] || fail "cursor stop output, got: $out"
jq -e 'select(.event=="start") | .cli=="cursor" and .id=="u-1" and .parent_id=="p1" and .name=="explore"
  and .transcript=="/c/agent-transcripts/p1/subagents/u-1.jsonl"' "$L" >/dev/null || fail "cursor start line wrong"
jq -e 'select(.event=="stop") | .id=="u-1" and .status=="error"' "$L" >/dev/null || fail "cursor stop line wrong"
cursor '{"hook_event_name":"subagentStop","status":"completed"}' >/dev/null   # no id derivable: skipped
out="$(cursor 'not json')"; [ "$out" = '{"permission":"allow"}' ] || fail "cursor bad JSON must still allow"
[ "$(wc -l < "$L" | tr -d ' ')" = 2 ] || fail "cursor skipped payloads wrote a line"

# opencode: events shaped from the SDK types (Session.parentID); not recorded live.
rm -f "$L"
node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
const info = (id, parentID) => ({ id, parentID, title: "Explore (@explore subagent)" });
await event({ event: { type: "session.created", properties: { info: info("ses_main") } } });
await event({ event: { type: "session.created", properties: { info: info("ses_c1", "ses_main") } } });
await event({ event: { type: "session.created", properties: { info: info("ses_c2", "ses_main") } } });
await event({ event: { type: "session.idle", properties: { sessionID: "ses_main" } } });
await event({ event: { type: "session.idle", properties: { sessionID: "ses_c1" } } });
await event({ event: { type: "session.idle", properties: { sessionID: "ses_c1" } } });
await event({ event: { type: "session.error", properties: { sessionID: "ses_c2" } } });
await event({ event: { type: "bogus" } });
await event({});
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
[ "$(wc -l < "$L" | tr -d ' ')" = 4 ] || fail "opencode: expected 2 start + 2 stop lines"
jq -e 'select(.event=="start" and .id=="ses_c1") | .cli=="opencode" and .parent_id=="ses_main" and .name=="Explore (@explore subagent)"' "$L" >/dev/null || fail "opencode start line wrong"
jq -e 'select(.event=="stop" and .id=="ses_c2") | .status=="error"' "$L" >/dev/null || fail "opencode error stop wrong"
jq -e 'select(.event=="stop" and .id=="ses_c1") | .status==null' "$L" >/dev/null || fail "opencode idle stop wrong"

echo "ok: cdamux sub-agent adapters"
