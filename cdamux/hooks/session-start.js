#!/usr/bin/env node
// cdamux — Claude Code SessionStart / Cursor sessionStart adapter. Stores the CLI's
// conversation id as agent_session_id in the restore record of the tmux session it
// runs in (ADR-009), so `cdamux restore` resumes the conversation that is live now
// (after /clear, /resume, a new chat). Only an existing record is updated.
// Fail-open: any error exits 0; local only.
const fs = require('fs');
const { setAgentSessionId } = require('./ledger');

let hookName = '';

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  hookName = input.hook_event_name;
  // Cursor documents session_id as "same as conversation_id".
  if (hookName === 'SessionStart') setAgentSessionId(process.env, input.session_id);
  else if (hookName === 'sessionStart') setAgentSessionId(process.env, input.session_id || input.conversation_id);
}

try { main(); } catch (e) { /* fail-open */ }
// Claude adds SessionStart stdout to the context: print nothing there. Cursor expects JSON.
if (hookName === 'sessionStart') process.stdout.write('{}\n');
process.exit(0);
