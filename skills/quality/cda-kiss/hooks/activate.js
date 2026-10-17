#!/usr/bin/env node
// kiss — SessionStart (Claude) / sessionStart (Cursor) hook.
// Injects the active-level ruleset + a cheap repo map as hidden session context.
// Fresh session → default level; "off" stays silent; compact keeps the live level.
const { readMode, setMode, getInstructions, buildMap, writeHookOutput, DEFAULT_MODE } = require('./lib');

function activate(source) {
  const live = readMode();
  const mode = live || (source === 'compact' ? 'off' : DEFAULT_MODE);
  if (mode === 'off') process.exit(0);
  setMode(mode);
  let out = getInstructions(mode);
  if (process.env.KISS_MAP !== '0') {
    const map = buildMap();
    if (map) out += '\n\n' + map;
  }
  try { writeHookOutput('SessionStart', out); } catch (e) { /* EPIPE at exit */ }
}

let input = '', done = false;
function finish() {
  if (done) return; done = true;
  let source = '';
  try { source = JSON.parse(input.replace(/^﻿/, '')).source; } catch (e) { /* fresh */ }
  activate(source);
}
process.stdin.on('data', c => { input += c; });
process.stdin.on('end', () => { finish(); process.exit(0); });
process.stdin.on('error', () => { finish(); process.exit(0); });
setTimeout(() => { finish(); process.exit(0); }, 1000);
