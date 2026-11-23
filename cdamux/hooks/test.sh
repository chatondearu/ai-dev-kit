#!/usr/bin/env bash
# cdamux sub-agent adapters (Claude, Cursor, opencode): sample payloads -> ledger lines. A stub `tmux` on
# PATH answers the session name, so no tmux server is touched.
set -euo pipefail
cd "$(dirname "$0")"
H="$PWD/claude-subagent.js"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
# tmux stub: `#{pane_pid} #S` -> "$PANE_PID fleet/demo", anything else -> the session name.
printf '#!/bin/sh\ncase "$*" in *pane_pid*) echo "$PANE_PID fleet/demo";; *) echo "fleet/demo";; esac\n' > "$tmp/bin/tmux"
# ps stub (`ps -o ppid=,comm= -p PID`): a fake ancestry from $CHAIN. The first (real) pid
# asked for has comm ${CHAIN[0]} and parent B+1; pid B+k has comm ${CHAIN[k]}, parent B+k+1.
cat > "$tmp/bin/ps" <<'PS'
#!/bin/sh
[ -z "$PS_FAIL" ] || exit 1
[ -z "$PS_LOG" ] || echo "$4 $PPID" >> "$PS_LOG"   # asked pid, caller (node) pid
B=90000000 pid="$4"; set -- $CHAIN
if [ "$pid" -lt "$B" ]; then i=0; else i=$((pid - B)); fi
[ "$i" -lt "$#" ] || exit 1
shift "$i"; echo "$((B + i + 1)) $1"
PS
chmod +x "$tmp/bin/tmux" "$tmp/bin/ps"
# macOS scans a new executable on its first exec; on a loaded host that outlasts the hooks'
# 1 s tmux/ps timeout and the first ledger line is lost. Warm both stubs (see cdamux below).
"$tmp/bin/tmux" >/dev/null; "$tmp/bin/ps" >/dev/null 2>&1 || true
# Run from a cdamux worker, the inherited home/agent would send fixtures into live state.
unset CDAMUX_HOME CDAMUX_AGENT_ID
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
# top-level: hook <- sh -c <- claude <- fish (login) <- fish (pane): one non-shell on the way
export CHAIN="sh claude fish -fish" PANE_PID=90000004

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

# only the pane's top-level CLI: a nested `claude -p` (hook <- sh <- claude <- zsh tool
# shell <- top claude <- pane), an orch worker under it, or a ps failure writes nothing
seed claude; before="$(cat "$R")"
CHAIN="sh claude zsh claude fish" PANE_PID=90000005 start '{"session_id":"nested","hook_event_name":"SessionStart"}' >/dev/null
CHAIN="sh claude bash bash claude zsh claude fish" PANE_PID=90000008 start '{"session_id":"nested","hook_event_name":"SessionStart"}' >/dev/null
PS_FAIL=1 start '{"session_id":"nested","hook_event_name":"SessionStart"}' >/dev/null
CHAIN="sh claude fish fish fish fish fish fish fish fish" PANE_PID=90000010 start '{"session_id":"nested","hook_event_name":"SessionStart"}' >/dev/null
[ "$(cat "$R")" = "$before" ] || fail "nested CLI / ps failure / pane out of reach changed the record"
CHAIN="claude fish" PANE_PID=90000002 start '{"session_id":"direct","hook_event_name":"SessionStart"}' >/dev/null
[ "$(jq -r .agent_session_id "$R")" = direct ] || fail "hook run without sh -c not accepted"

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

# opencode: the main session (no parentID) sets the id; a child session does not.
# In-process plugin: the walk starts at opencode itself (opencode <- fish pane).
seed opencode
CHAIN="opencode fish" PANE_PID=90000002 PS_LOG="$tmp/ps.log" node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
await event({ event: { type: "session.created", properties: { info: { id: "ses_main", title: "x" } } } });
await event({ event: { type: "session.created", properties: { info: { id: "ses_c1", parentID: "ses_main" } } } });
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
jq -e '.agent_session_id=="ses_main"' "$R" >/dev/null || fail "opencode main id not stored"
head -n 1 "$tmp/ps.log" | awk '$1 != $2 { exit 1 }' || fail "opencode walk must start at its own pid"
kept opencode
CHAIN="opencode zsh opencode fish" PANE_PID=90000004 node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
await event({ event: { type: "session.created", properties: { info: { id: "ses_nested" } } } });
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
jq -e '.agent_session_id=="ses_main"' "$R" >/dev/null || fail "nested opencode overwrote the id"

# Status hooks (ADR-011 §3): stub `cdamux` logs argv and merges status into the agent file.
ST="$PWD/status.js"
AF="$tmp/state/cdamux/agents/a-hook1.json"
cat > "$tmp/bin/merge-status.js" <<'JS'
const fs = require('fs');
const f = process.argv[2];
const p = JSON.parse(process.argv[3]);
let o = {};
try { o = JSON.parse(fs.readFileSync(f, 'utf8')); } catch (e) { /* create */ }
Object.assign(o, p);
fs.mkdirSync(require('path').dirname(f), { recursive: true });
fs.writeFileSync(f, JSON.stringify(o));
JS
cat > "$tmp/bin/cdamux" <<CD
#!/bin/sh
# The detached state tick lands whenever it likes: keep it out of the asserted log.
[ "\$1 \$2" = "state tick" ] && { echo tick >> "\$CDAMUX_LOG.tick"; exit 0; }
echo "\$@" >> "\$CDAMUX_LOG"
if [ "\$1 \$2 \$3" = "msg read --ack" ]; then
  # Stub of the Rust reader: print unread lines, move the cursor to the end.
  [ -z "\$STUB_READ_FAIL" ] || exit 1
  d="\$XDG_STATE_HOME/cdamux/inbox"; c=\$(cat "\$d/\$5.cursor" 2>/dev/null || echo 0)
  tail -c +\$((c + 1)) "\$d/\$5.jsonl"; wc -c < "\$d/\$5.jsonl" | tr -d ' ' > "\$d/\$5.cursor"
  exit 0
fi
[ "\$1 \$2 \$3" = "state append agents" ] || exit 0
f="\$XDG_STATE_HOME/cdamux/agents/\$4.json"
node "$tmp/bin/merge-status.js" "\$f" "\$5"
CD
chmod +x "$tmp/bin/cdamux"
export CDAMUX_AGENT_ID=a-hook1 CDAMUX_LOG="$tmp/cdamux.log"
# macOS scans a new executable on its first exec (~250 ms): warm the stub so the hooks'
# 500 ms timeout measures the stub, not the scan.
"$tmp/bin/cdamux" warm
printf '{"id":"a-hook1","status":"done"}\n' > "$AF"
run_status() { printf '%s' "$1" | node "$ST" || fail "status hook exited non-zero"; }
: > "$CDAMUX_LOG"
run_status '{"hook_event_name":"Stop"}' >/dev/null
grep -q '"status"' "$CDAMUX_LOG" && fail "sticky done should block idle write"
# `turn` is not sticky (ADR-018): the turn end is recorded under any status.
[ "$(jq -c '[.status, .turn]' "$AF")" = '["done","ended"]' ] || fail "Stop must record turn ended, got: $(cat "$AF")"
# A pending ask (blocked) survives the asker's own turn end; the next prompt clears it below.
# A permission prompt under it is still recorded as a running turn: no wake line lands there.
printf '{"id":"a-hook1","status":"blocked"}\n' > "$AF"
run_status '{"hook_event_name":"Stop"}' >/dev/null
run_status '{"hook_event_name":"Notification","notification_type":"permission_prompt","message":"Claude needs your permission to use Bash"}' >/dev/null
grep -q '"status"' "$CDAMUX_LOG" && fail "sticky blocked should block idle/needs_input writes"
[ "$(jq -c '[.status, .turn]' "$AF")" = '["blocked","running"]' ] || fail "permission prompt must mark the turn running, got: $(cat "$AF")"
printf '{"id":"a-hook1","status":"blocked","turn":"ended"}\n' > "$AF"
run_status '{"hook_event_name":"PermissionRequest"}' >/dev/null
[ "$(jq -r .turn "$AF")" = running ] || fail "PermissionRequest must mark the turn running"
out="$(run_status '{"hook_event_name":"beforeSubmitPrompt"}')"
[ "$out" = '{"continue":true}' ] || fail "cursor beforeSubmitPrompt output, got: $out"
grep -q 'state append agents a-hook1 {"status":"working","turn":"running"}' "$CDAMUX_LOG" || fail "working not written"
[ "$(jq -r .status "$AF")" = working ] || fail "agent file status not working"
grep -q 'state append agents a-hook1 {"status":"working","turn":"running"} --sticky' "$CDAMUX_LOG" || fail "status write must pass --sticky"
for _ in 1 2 3 4 5 6 7 8 9 10; do [ -s "$CDAMUX_LOG.tick" ] && break; sleep 0.1; done
[ -s "$CDAMUX_LOG.tick" ] || fail "status write must kick cdamux state tick"
# An older cdamux without --sticky exits 2: the hook retries the plain append.
cp "$tmp/bin/cdamux" "$tmp/bin/cdamux.new"
cat > "$tmp/bin/cdamux" <<CD
#!/bin/sh
case "\$*" in *--sticky*) echo "\$@" >> "\$CDAMUX_LOG"; exit 2;; esac
exec "$tmp/bin/cdamux.new" "\$@"
CD
"$tmp/bin/cdamux" warm
printf '{"id":"a-hook1","status":"idle"}\n' > "$AF"; : > "$CDAMUX_LOG"
run_status '{"hook_event_name":"beforeSubmitPrompt"}' >/dev/null
[ "$(jq -r .status "$AF")" = working ] || fail "old binary fallback did not write working"
mv "$tmp/bin/cdamux.new" "$tmp/bin/cdamux"
run_status '{"hook_event_name":"Notification","notification_type":"permission_prompt","message":"Claude needs your permission to use Bash"}' >/dev/null
[ "$(jq -r .status "$AF")" = needs_input ] || fail "needs_input not sticky-cleared after working"
# Claude's idle_prompt Notification (~60 s after a turn ends) is not needs_input: an idle
# agent must stay idle so the orchestrator nudge still fires (msg.rs should_nudge).
printf '{"id":"a-hook1","status":"idle"}\n' > "$AF"; : > "$CDAMUX_LOG"
run_status '{"hook_event_name":"Notification","notification_type":"idle_prompt","message":"Claude is waiting for your input"}' >/dev/null
run_status '{"hook_event_name":"Notification","message":"Claude is waiting for your input"}' >/dev/null
[ ! -s "$CDAMUX_LOG" ] && [ "$(jq -r .status "$AF")" = idle ] || fail "idle_prompt Notification must not write needs_input"
# Untyped payload (older clients): only a permission message maps to needs_input.
run_status '{"hook_event_name":"Notification","message":"Claude needs your permission to use Bash"}' >/dev/null
[ "$(jq -r .status "$AF")" = needs_input ] || fail "untyped permission Notification not needs_input"
out="$(run_status '{"hook_event_name":"sessionEnd"}')"
[ "$out" = '{}' ] || fail "sessionEnd must print {}, got: $out"
[ "$(jq -r .status "$AF")" = idle ] || fail "sessionEnd did not set idle"
[ "$(jq -r .turn "$AF")" = exited ] || fail "sessionEnd must mark the turn exited (pane may be a shell)"
# No agent id → no-op (also drop TMUX so the stub cannot fake @cdamux_id)
: > "$CDAMUX_LOG"
( unset CDAMUX_AGENT_ID TMUX; run_status '{"hook_event_name":"UserPromptSubmit"}' >/dev/null )
[ ! -s "$CDAMUX_LOG" ] || fail "status wrote without agent id"
# No cdamux on PATH → fail-open exit 0 (keep host node; drop the stub bin dir)
PATH="$(dirname "$(command -v node)"):/usr/bin:/bin" run_status '{"hook_event_name":"UserPromptSubmit"}' >/dev/null || fail "missing cdamux must still exit 0"

# Inbox hand-over (ADR-018): a Claude turn end with unread messages blocks the stop with
# the full bodies as the reason and acks them; the turn goes on (no idle, no turn end).
INBOX="$tmp/state/cdamux/inbox"
mkdir -p "$INBOX"
seed_inbox() {
  printf '{"id":"m-1","ts":1,"from":"a-w","to":"a-hook1","kind":"relay","text":"pr opened\\nsecond line"}\n' > "$INBOX/a-hook1.jsonl"
  printf '0\n' > "$INBOX/a-hook1.cursor"
}
seed_inbox
printf '{"id":"a-hook1","client":"claude","status":"working","turn":"running"}\n' > "$AF"; : > "$CDAMUX_LOG"
out="$(run_status '{"hook_event_name":"Stop"}')"
echo "$out" | jq -e '.decision=="block"
  and (.reason|test("1 message"))
  and (.reason|test("relay from a-w · m-1"))
  and (.reason|test("pr opened\nsecond line"))' >/dev/null \
  || fail "Stop must block with the full bodies, got: $out"
[ "$(cat "$INBOX/a-hook1.cursor")" = "$(wc -c < "$INBOX/a-hook1.jsonl" | tr -d ' ')" ] || fail "Stop hand-over must ack"
[ "$(jq -c '[.status, .turn]' "$AF")" = '["working","running"]' ] || fail "blocked Stop must keep the turn running, got: $(cat "$AF")"
# Nothing left (acked): the re-fired Stop (stop_hook_active) ends the turn normally.
out="$(run_status '{"hook_event_name":"Stop","stop_hook_active":true}')"
[ -z "$out" ] || fail "Stop with an empty inbox must stay silent, got: $out"
[ "$(jq -c '[.status, .turn]' "$AF")" = '["idle","ended"]' ] || fail "Stop must end the turn, got: $(cat "$AF")"
# A line that arrived during the continued turn is handed over too, even when stop_hook_active.
seed_inbox
out="$(run_status '{"hook_event_name":"Stop","stop_hook_active":true}')"
echo "$out" | jq -e '.decision=="block"' >/dev/null || fail "new unread must block again, got: $out"
# A failed read never blocks (it could not ack): the stop goes through, the line stays unread.
seed_inbox
out="$(STUB_READ_FAIL=1 run_status '{"hook_event_name":"Stop"}')"
[ -z "$out" ] || fail "failed read must not block, got: $out"
[ "$(cat "$INBOX/a-hook1.cursor")" = 0 ] || fail "failed read must leave the line unread"
# A nested CLI (`claude -p` from a tool call) inherits the agent id: no hand-over, no ack,
# no status / turn write (it took the parent's inbox and set it idle mid-turn).
nested() { CHAIN="sh claude zsh claude fish" PANE_PID=90000005 run_status "$1"; }
printf '{"id":"a-hook1","client":"claude","status":"working","turn":"running"}\n' > "$AF"; : > "$CDAMUX_LOG"
before="$(cat "$AF")"
[ -z "$(nested '{"hook_event_name":"UserPromptSubmit"}')" ] || fail "nested UserPromptSubmit must print nothing"
[ -z "$(nested '{"hook_event_name":"Stop"}')" ] || fail "nested Stop must not block"
[ "$(nested '{"hook_event_name":"beforeSubmitPrompt"}')" = '{"continue":true}' ] || fail "nested Cursor hook must answer continue"
[ "$(cat "$INBOX/a-hook1.cursor")" = 0 ] || fail "nested hook acked the parent's inbox"
[ ! -s "$CDAMUX_LOG" ] && [ "$(cat "$AF")" = "$before" ] || fail "nested hook wrote the parent's state: $(cat "$CDAMUX_LOG")"
# Consecutive Stop blocks are capped (3): then the turn ends and the line stays unread.
for active in false true true; do
  seed_inbox
  out="$(run_status "{\"hook_event_name\":\"Stop\",\"stop_hook_active\":$active}")"
  echo "$out" | jq -e '.decision=="block"' >/dev/null || fail "Stop block $active under the cap, got: $out"
done
seed_inbox
out="$(run_status '{"hook_event_name":"Stop","stop_hook_active":true}')"
[ -z "$out" ] || fail "4th consecutive Stop must not block, got: $out"
[ "$(cat "$INBOX/a-hook1.cursor")" = 0 ] && [ "$(jq -r .turn "$AF")" = ended ] || fail "capped Stop must end the turn and leave the line unread"
# A fresh stop resets the count.
out="$(run_status '{"hook_event_name":"Stop"}')"
echo "$out" | jq -e '.decision=="block"' >/dev/null || fail "fresh Stop must block again, got: $out"
seed_inbox
# Prompt submit (wake line or human prompt) injects the bodies and acks them.
out="$(run_status '{"hook_event_name":"UserPromptSubmit"}')"
echo "$out" | jq -e '.hookSpecificOutput.hookEventName=="UserPromptSubmit"
  and (.hookSpecificOutput.additionalContext|test("pr opened\nsecond line"))' >/dev/null \
  || fail "UserPromptSubmit must inject the bodies, got: $out"
[ "$(cat "$INBOX/a-hook1.cursor")" != 0 ] || fail "UserPromptSubmit hand-over must ack"
# Other clients on Claude-shaped events (codex): unacked digest, the turn ends normally.
seed_inbox
printf '{"id":"a-hook1","client":"codex","status":"working","turn":"running"}\n' > "$AF"
out="$(run_status '{"hook_event_name":"Stop"}')"
echo "$out" | jq -e '.hookSpecificOutput.hookEventName=="Stop" and (.hookSpecificOutput.additionalContext|test("1 unread"))' >/dev/null \
  || fail "codex Stop must get the digest, got: $out"
out="$(run_status '{"hook_event_name":"UserPromptSubmit"}')"
echo "$out" | jq -e '.hookSpecificOutput.additionalContext|test("msg read --ack")' >/dev/null \
  || fail "codex UserPromptSubmit must get the digest, got: $out"
[ "$(cat "$INBOX/a-hook1.cursor")" = 0 ] || fail "codex hand-over must not ack"
# A resumed / cleared Claude session fires no Stop for the turn that died: turn ended.
printf '{"id":"a-hook1","client":"claude","status":"working","turn":"running"}\n' > "$AF"
start '{"session_id":"c-9","hook_event_name":"SessionStart","source":"compact"}' >/dev/null
[ "$(jq -r .turn "$AF")" = running ] || fail "compaction must not end the turn"
start '{"session_id":"c-9","hook_event_name":"SessionStart","source":"resume"}' >/dev/null
[ "$(jq -r .turn "$AF")" = ended ] || fail "resume must end the turn"
# Cursor cannot inject: its stop hook keeps the digest followup (agent runs msg read --ack).
seed_inbox
out="$(run_status '{"hook_event_name":"stop","loop_count":0}')"
echo "$out" | jq -e '.followup_message|test("unread")' >/dev/null \
  || fail "cursor stop followup_message missing, got: $out"
# The queued followup runs without beforeSubmitPrompt: the turn goes on, stay working.
[ "$(jq -r .status "$AF")" = working ] || fail "cursor stop with followup must stay working"
out="$(run_status '{"hook_event_name":"stop","loop_count":1}')"
[ "$out" = '{}' ] || fail "cursor stop loop_count>0 must not follow up, got: $out"
[ "$(jq -r .status "$AF")" = idle ] || fail "cursor stop without followup must set idle"
# An open ask (blocked) survives a stop that queues the digest; the followup is still sent.
printf '{"id":"a-hook1","status":"blocked"}\n' > "$AF"
out="$(run_status '{"hook_event_name":"stop","loop_count":0}')"
echo "$out" | jq -e '.followup_message|test("unread")' >/dev/null \
  || fail "cursor stop followup_message missing when blocked, got: $out"
[ "$(jq -r .status "$AF")" = blocked ] || fail "cursor stop followup cleared sticky blocked"
printf '{"id":"a-hook1","status":"idle"}\n' > "$AF"
# postToolUse: a tool ran, so a queued turn is live; lift idle only, never a sticky status.
out="$(run_status '{"hook_event_name":"postToolUse"}')"
[ "$out" = '{}' ] || fail "postToolUse must print {}, got: $out"
[ "$(jq -r .status "$AF")" = working ] || fail "postToolUse did not lift idle to working"
printf '{"id":"a-hook1","status":"done"}\n' > "$AF"
run_status '{"hook_event_name":"postToolUseFailure"}' >/dev/null
[ "$(jq -r .status "$AF")" = done ] || fail "postToolUse must not clear sticky done"
# beforeSubmitPrompt still cannot inject (Cursor limit)
out="$(run_status '{"hook_event_name":"beforeSubmitPrompt"}')"
[ "$out" = '{"continue":true}' ] || fail "beforeSubmitPrompt must not inject, got: $out"
# Acked inbox → no digest
printf '%s' "$(wc -c < "$INBOX/a-hook1.jsonl" | tr -d ' ')" > "$INBOX/a-hook1.cursor"
out="$(run_status '{"hook_event_name":"Stop"}')"
[ -z "$out" ] || fail "acked inbox must yield empty Stop output, got: $out"
# Events feed: an orchestrator's turn end hands the payload to `cdamux events reply`;
# a worker's does not (no extra process per worker turn).
grep -q 'events reply' "$CDAMUX_LOG" && fail "worker Stop must not record a reply"
printf '{"id":"a-hook1","status":"idle","role":"orchestrator"}\n' > "$AF"
run_status '{"hook_event_name":"Stop","transcript_path":"/t.jsonl"}' >/dev/null
grep -q 'events reply a-hook1' "$CDAMUX_LOG" || fail "orchestrator Stop did not record a reply"
# `cdamux say` queue: an orchestrator's turn end drains it only when a prompt is queued;
# a worker's, an empty queue and a turn that continues (Cursor followup) do not spawn it.
SAYQ="$tmp/state/cdamux/say/a-hook1.jsonl"
: > "$CDAMUX_LOG"
run_status '{"hook_event_name":"Stop"}' >/dev/null
grep -q 'msg drain' "$CDAMUX_LOG" && fail "empty say queue must not spawn a drain"
mkdir -p "$(dirname "$SAYQ")"; printf '{"id":"m-s1","ts":1,"from":"alice","text":"hi"}\n' > "$SAYQ"
run_status '{"hook_event_name":"Stop"}' >/dev/null
grep -q 'msg drain a-hook1' "$CDAMUX_LOG" || fail "orchestrator Stop did not drain the say queue"
: > "$CDAMUX_LOG"
run_status '{"hook_event_name":"UserPromptSubmit"}' >/dev/null
grep -q 'msg drain' "$CDAMUX_LOG" && fail "a running turn must not drain"
printf '{"id":"a-hook1","status":"idle","role":"worker"}\n' > "$AF"; : > "$CDAMUX_LOG"
run_status '{"hook_event_name":"Stop"}' >/dev/null
grep -q 'msg drain' "$CDAMUX_LOG" && fail "worker Stop must not drain"
rm -f "$SAYQ"

# opencode status: `session.status` busy on the main session writes working; a child's busy does not.
printf '{"id":"a-hook1","status":"idle"}\n' > "$AF"
: > "$CDAMUX_LOG"
node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
await event({ event: { type: "session.created", properties: { info: { id: "ses_main" } } } });
await event({ event: { type: "session.created", properties: { info: { id: "ses_c1", parentID: "ses_main" } } } });
await event({ event: { type: "session.status", properties: { sessionID: "ses_c1", status: { type: "busy" } } } });
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
[ "$(jq -r .status "$AF")" = idle ] || fail "opencode child busy changed the main status"
node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
await event({ event: { type: "session.created", properties: { info: { id: "ses_main" } } } });
await event({ event: { type: "session.status", properties: { sessionID: "ses_main", status: { type: "busy" } } } });
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
[ "$(jq -r .status "$AF")" = working ] || fail "opencode busy not written as working"
node --input-type=module -e '
const { CdamuxSubagent } = await import(process.argv[1]);
const { event } = await CdamuxSubagent();
await event({ event: { type: "session.created", properties: { info: { id: "ses_main" } } } });
await event({ event: { type: "session.idle", properties: { sessionID: "ses_main" } } });
' "$PWD/opencode-subagent.js" || fail "opencode plugin threw"
[ "$(jq -r .status "$AF")" = idle ] || fail "opencode idle not written after busy"

# A 1s events-reply timeout must retry once (no transcript) and still exit 0.
# The first reply sleeps; node kills it. Today, with the retry removed, only one call is logged.
printf '{"id":"a-hook1","status":"idle","role":"orchestrator"}\n' > "$AF"
: > "$CDAMUX_LOG"
cat > "$tmp/bin/cdamux" <<CD
#!/usr/bin/env node
const fs = require('fs');
const log = process.env.CDAMUX_LOG;
const dir = process.env.REPLY_DIR;
fs.appendFileSync(log, process.argv.slice(2).join(' ') + '\n');
if (process.argv[2] !== 'events' || process.argv[3] !== 'reply') process.exit(0);
const nfile = dir + '/n';
let n = 0;
try { n = Number(fs.readFileSync(nfile, 'utf8')); } catch (e) { /* first */ }
n += 1;
fs.writeFileSync(nfile, String(n));
const chunks = [];
process.stdin.on('data', (c) => chunks.push(c));
process.stdin.on('end', () => {
  fs.writeFileSync(dir + '/stdin-' + n, Buffer.concat(chunks));
  if (n === 1) setTimeout(() => process.exit(0), 5000);
  else process.exit(0);
});
CD
chmod +x "$tmp/bin/cdamux"
export REPLY_DIR="$tmp/reply-slow"
mkdir -p "$REPLY_DIR"
out="$(run_status '{"hook_event_name":"stop","generation_id":"g-slow","last_assistant_message":"","transcript_path":"/slow.jsonl","loop_count":1}')"
[ "$out" = '{}' ] || fail "timed-out stop must still print {}, got: $out"
[ "$(cat "$REPLY_DIR/n")" = 2 ] || fail "timeout must retry events reply once, got: $(cat "$REPLY_DIR/n" 2>/dev/null || echo 0)"
jq -e '.transcript_path == "/slow.jsonl" and .generation_id == "g-slow"' "$REPLY_DIR/stdin-1" >/dev/null \
  || fail "first events reply dropped the payload"
jq -e '.generation_id == "g-slow" and (has("transcript_path") | not)' "$REPLY_DIR/stdin-2" >/dev/null \
  || fail "timeout retry must drop transcript_path, got: $(cat "$REPLY_DIR/stdin-2")"

echo "ok: cdamux sub-agent + session id + status adapters"
