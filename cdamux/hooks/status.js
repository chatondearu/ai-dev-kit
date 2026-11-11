#!/usr/bin/env node
// cdamux — status adapter for Claude / Cursor / Codex hooks (ADR-011 §3).
// Maps lifecycle events → `cdamux state append agents <id> {"status":…}`.
// Fail-open, local, fast; no-op without CDAMUX_AGENT_ID / @cdamux_id or without
// `cdamux` on PATH. Stickiness: done/error stay until the next `working`.
//
// Cursor: sessionStart/sessionEnd → idle; beforeSubmitPrompt → working when fired.
// Print-mode (`cursor-agent -p`) may omit beforeSubmitPrompt — rely on sessionStart/End.
const fs = require('fs');
const { setAgentStatus } = require('./ledger');

const MAP = {
  UserPromptSubmit: 'working',
  beforeSubmitPrompt: 'working',
  Stop: 'idle',
  SessionStart: 'idle',
  sessionStart: 'idle',
  SessionEnd: 'idle',
  sessionEnd: 'idle',
  Notification: 'needs_input',
  PermissionRequest: 'needs_input',
};

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  const event = input.hook_event_name || input.event || '';
  const status = MAP[event];
  if (status) setAgentStatus(process.env, status);
  if (event === 'beforeSubmitPrompt') process.stdout.write('{"continue":true}\n');
  else if (event === 'sessionStart' || event === 'sessionEnd') process.stdout.write('{}\n');
}

try { main(); } catch (e) { /* fail-open */ }
process.exit(0);
