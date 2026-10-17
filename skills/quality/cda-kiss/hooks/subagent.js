#!/usr/bin/env node
// kiss — SubagentStart hook (Claude). SessionStart context never reaches
// Task-spawned agents, so inject the same ruleset into each subagent when kiss
// is active. That's how cda-dev workers write lean by default.
const { readMode, getInstructions, writeHookOutput } = require('./lib');

const mode = readMode();
if (!mode || mode === 'off') process.exit(0);
try { writeHookOutput('SubagentStart', getInstructions(mode)); } catch (e) { /* EPIPE */ }
process.exit(0);
