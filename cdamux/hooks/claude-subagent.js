#!/usr/bin/env node
// cdamux — Claude Code SubagentStart/SubagentStop adapter. Appends one line to the
// sub-agent ledger of the tmux session this Claude runs in (schema:
// cdamux/docs/subagents.md). Fail-open: any error exits 0 silently; local only.
const fs = require('fs');
const path = require('path');
const { append, expandHome } = require('./ledger');

const EVENTS = { SubagentStart: 'start', SubagentStop: 'stop' };

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  const event = EVENTS[input.hook_event_name];
  // Empty agent_type = Claude's internal agents (prompt suggestions, /btw): not sub-agents.
  if (!event || !input.agent_id || !input.agent_type) return;
  // Docs show agent_id both with and without the `agent-` prefix; the transcript file
  // is always subagents/agent-<id>.jsonl. Normalize so start and stop share one id.
  const id = String(input.agent_id).replace(/^agent-/, '');
  let transcript = expandHome(input.agent_transcript_path);
  if (!transcript && input.transcript_path && input.session_id) {
    // SubagentStart has no agent_transcript_path: derive it next to the main transcript.
    transcript = path.join(path.dirname(expandHome(input.transcript_path)), String(input.session_id), 'subagents', `agent-${id}.jsonl`);
  }
  append(process.env, {
    cli: 'claude',
    event,
    id,
    parent_id: input.session_id || undefined,
    name: String(input.agent_type),
    transcript: transcript || undefined,
  });
}

try { main(); } catch (e) { /* fail-open */ }
process.exit(0);
