#!/usr/bin/env node
// cdamux — Cursor subagentStart/subagentStop adapter. Appends one line to the
// sub-agent ledger of the tmux session cursor-agent runs in (schema:
// cdamux/docs/subagents.md). Fail-open: any error still answers "allow", exit 0.
const fs = require('fs');
const path = require('path');
const { append, expandHome } = require('./ledger');

let hookName = '';

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  hookName = input.hook_event_name;
  const event = { subagentStart: 'start', subagentStop: 'stop' }[hookName];
  if (!event) return;
  // subagentStart carries subagent_id but no transcript; subagentStop carries the
  // transcript but no id. Cursor names the file subagents/<subagent_id>.jsonl, next
  // to the parent transcript, so the file name is the id that links the two lines.
  let transcript = expandHome(input.agent_transcript_path);
  if (!transcript && input.transcript_path && input.subagent_id) {
    transcript = path.join(path.dirname(expandHome(input.transcript_path)), 'subagents', `${input.subagent_id}.jsonl`);
  }
  const id = event === 'start' ? input.subagent_id : transcript && path.basename(transcript, '.jsonl');
  if (!id) return;
  append(process.env, {
    cli: 'cursor',
    event,
    id: String(id),
    parent_id: input.parent_conversation_id || undefined,
    name: input.subagent_type ? String(input.subagent_type) : undefined,
    transcript: transcript || undefined,
    status: event === 'stop' && input.status === 'error' ? 'error' : undefined,
  });
}

try { main(); } catch (e) { /* fail-open */ }
// subagentStart is a permission hook: empty or invalid output would block the sub-agent.
process.stdout.write(hookName === 'subagentStop' ? '{}\n' : '{"permission":"allow"}\n');
process.exit(0);
