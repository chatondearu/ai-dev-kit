#!/usr/bin/env node
// cdamux — status adapter for Claude / Cursor / Codex hooks (ADR-011 §3 + §4).
// Maps lifecycle events → `cdamux state append agents <id> {"status":…}`.
// Turn-end / prompt-submit inject an unread inbox digest where the client allows it.
// Fail-open, local, fast; no-op without CDAMUX_AGENT_ID / @cdamux_id or without
// `cdamux` on PATH. Stickiness: done/error/blocked stay until the next `working`.
//
// Cursor: sessionStart/sessionEnd → idle; beforeSubmitPrompt → working when fired
// (cannot inject context — see docs); stop → idle + followup_message digest.
// Print-mode (`cursor-agent -p`) may omit beforeSubmitPrompt — rely on sessionStart/End.
const fs = require('fs');
const { setAgentStatus, unreadDigest } = require('./ledger');

const MAP = {
  UserPromptSubmit: 'working',
  beforeSubmitPrompt: 'working',
  Stop: 'idle',
  stop: 'idle',
  SessionStart: 'idle',
  sessionStart: 'idle',
  SessionEnd: 'idle',
  sessionEnd: 'idle',
  Notification: 'needs_input',
  PermissionRequest: 'needs_input',
};

function writeJson(obj) {
  process.stdout.write(JSON.stringify(obj) + '\n');
}

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  const event = input.hook_event_name || input.event || '';
  const status = MAP[event];
  if (status) setAgentStatus(process.env, status);

  const digest = unreadDigest(process.env);

  if (event === 'beforeSubmitPrompt') {
    // Cursor cannot inject additional_context here (continue / user_message only).
    writeJson({ continue: true });
    return;
  }
  if (event === 'UserPromptSubmit') {
    if (digest) {
      writeJson({
        hookSpecificOutput: {
          hookEventName: 'UserPromptSubmit',
          additionalContext: digest,
        },
      });
    }
    return;
  }
  if (event === 'Stop') {
    if (digest && !input.stop_hook_active) {
      writeJson({
        hookSpecificOutput: {
          hookEventName: 'Stop',
          additionalContext: digest,
        },
      });
    }
    return;
  }
  if (event === 'stop') {
    // Cursor stop: followup_message continues the turn. Cap at first auto-loop.
    const loop = Number(input.loop_count) || 0;
    if (digest && loop === 0) {
      writeJson({ followup_message: digest });
    } else {
      writeJson({});
    }
    return;
  }
  if (event === 'sessionStart' || event === 'sessionEnd') {
    writeJson({});
  }
}

try { main(); } catch (e) { /* fail-open */ }
process.exit(0);
