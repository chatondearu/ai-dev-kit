#!/usr/bin/env node
// kiss — UserPromptSubmit (Claude) / beforeSubmitPrompt (Cursor) hook.
// Watches the prompt for `/kiss [lite|full|ultra|off]`, "stop kiss" or
// "normal mode" and switches the persisted level. No command → no-op.
const { parseCommand, setMode, getInstructions, writeHookOutput, isCursor } = require('./lib');

let input = '', done = false;
function finish() {
  if (done) return; done = true;
  let prompt = '';
  try { prompt = JSON.parse(input.replace(/^﻿/, '')).prompt || ''; } catch (e) { /* no payload */ }
  const cmd = parseCommand(prompt);
  if (!cmd) { if (isCursor) writeHookOutput('beforeSubmitPrompt', ''); return; }
  if (cmd.mode === 'off') {
    setMode('off'); // sticky: next SessionStart stays silent until /kiss
    writeHookOutput('UserPromptSubmit', 'KISS MODE OFF — back to normal. Re-enable with `/kiss`.');
  } else {
    setMode(cmd.mode);
    writeHookOutput('UserPromptSubmit', getInstructions(cmd.mode));
  }
}
process.stdin.on('data', c => { input += c; });
process.stdin.on('end', () => { finish(); process.exit(0); });
process.stdin.on('error', () => { finish(); process.exit(0); });
setTimeout(() => { finish(); process.exit(0); }, 1000);
