#!/usr/bin/env node
// cdamux — Claude Code SessionStart / Cursor sessionStart adapter. Stores the CLI's
// conversation id as agent_session_id in the restore record of the tmux session it
// runs in (ADR-009), so `cdamux restore` resumes the conversation that is live now
// (after /clear, /resume, a new chat). Only an existing record is updated.
// Also sets agent status to idle (ADR-011 §3), and `turn: ended` after a resume or /clear
// (a restored session fires no Stop for the turn that died; ADR-018). Never at startup
// (the first prompt follows) or compaction (it can run mid-turn).
// Fail-open: any error exits 0; local only.
const fs = require('fs');
const { setAgentSessionId, setAgentState } = require('./ledger');

let hookName = '';

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  hookName = input.hook_event_name;
  // Cursor documents session_id as "same as conversation_id".
  if (hookName === 'SessionStart') setAgentSessionId(process.env, input.session_id);
  else if (hookName === 'sessionStart') setAgentSessionId(process.env, input.session_id || input.conversation_id);
  const ended = hookName === 'SessionStart' && (input.source === 'resume' || input.source === 'clear');
  setAgentState(process.env, 'idle', ended ? 'ended' : null);
}

try { main(); } catch (e) { /* fail-open */ }
// Claude adds SessionStart stdout to the context: print nothing there. Cursor expects JSON.
if (hookName === 'sessionStart') process.stdout.write('{}\n');
process.exit(0);
