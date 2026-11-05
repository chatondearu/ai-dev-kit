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

# Session id capture (restore registry, ADR-009): SessionStart -> agent_session_id.
# Claude payload shaped from the hook docs (SessionStart: session_id, source); Cursor from
# https://cursor.com/docs/hooks (sessionStart: session_id = conversation_id). Not recorded live.
S="$PWD/session-start.js"
R="$tmp/state/cdamux/sessions/fleet_demo.json"
mkdir -p "$(dirname "$R")"
seed() { printf '{"name":"fleet/demo","cwd":"/src/app","agent":"%s","agent_session_id":null,"parent":"fleet","role":"agent","layout":"single","created_ts":100,"updated_ts":100,"extra":1}\n' "$1" > "$R"; }
kept() { jq -e '.name=="fleet/demo" and .cwd=="/src/app" and .parent=="fleet" and .role=="agent" and .layout=="single" and .created_ts==100 and .extra==1' "$R" >/dev/null || fail "$1: other keys not kept"; }
start() { printf '%s' "$1" | node "$S" || fail "session-start exited non-zero"; }

seed claude
out="$(start '{"session_id":"c-1","transcript_path":"/p/c-1.jsonl","cwd":"/src/app","hook_event_name":"SessionStart","source":"startup"}')"
[ -z "$out" ] || fail "claude SessionStart must print nothing, got: $out"
jq -e '.agent_session_id=="c-1" and .updated_ts > 100' "$R" >/dev/null || fail "claude id not stored"
kept claude
start '{"session_id":"c-2","hook_event_name":"SessionStart","source":"clear"}' >/dev/null
[ "$(jq -r .agent_session_id "$R")" = c-2 ] || fail "claude /clear id not stored"
ls "$(dirname "$R")" | grep -q tmp && fail "tmp file left behind"

# skipped: other events, bad JSON, outside tmux, no id; a bad record is left as is
before="$(cat "$R")"
start '{"session_id":"x","hook_event_name":"SubagentStart"}' >/dev/null
start 'not json' >/dev/null
printf '%s' '{"session_id":"x","hook_event_name":"SessionStart"}' | env -u TMUX node "$S" || fail "outside tmux exit"
start '{"hook_event_name":"SessionStart"}' >/dev/null
[ "$(cat "$R")" = "$before" ] || fail "skipped payloads changed the record"
printf '{ broken' > "$R"
start '{"session_id":"x","hook_event_name":"SessionStart"}' >/dev/null
[ "$(cat "$R")" = '{ broken' ] || fail "malformed record rewritten"

# a session cdamux did not spawn (no record) gets none
rm -f "$R"
start '{"session_id":"x","hook_event_name":"SessionStart"}' >/dev/null
[ ! -e "$R" ] || fail "record created for a non-cdamux session"

seed cursor
out="$(start '{"conversation_id":"cu-1","generation_id":"g","hook_event_name":"sessionStart","session_id":"cu-1","is_background_agent":false,"composer_mode":"agent"}')"
[ "$out" = '{}' ] || fail "cursor sessionStart must print {}, got: $out"
jq -e '.agent_session_id=="cu-1"' "$R" >/dev/null || fail "cursor id not stored"
kept cursor
out="$(start 'not json')"; [ -z "$out" ] || fail "bad JSON printed output"

# opencode: the main session (no parentID) sets the id; a child session does not
seed opencode
node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
await event({ event: { type: "session.created", properties: { info: { id: "ses_main", title: "x" } } } });
await event({ event: { type: "session.created", properties: { info: { id: "ses_c1", parentID: "ses_main" } } } });
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
jq -e '.agent_session_id=="ses_main"' "$R" >/dev/null || fail "opencode main id not stored"
kept opencode

echo "ok: cdamux sub-agent + session id adapters"
