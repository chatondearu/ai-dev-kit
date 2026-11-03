#!/usr/bin/env node
// cdamux — Claude Code SubagentStart/SubagentStop adapter. Appends one line to the
// sub-agent ledger of the tmux session this Claude runs in (schema:
// cdamux/docs/subagents.md). Fail-open: any error exits 0 silently; local only.
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');

const MAX_LINES = 500;
const EVENTS = { SubagentStart: 'start', SubagentStop: 'stop' };

const expandHome = (p) => (p && p.startsWith('~/') ? path.join(os.homedir(), p.slice(2)) : p);

function tmuxSession(env) {
  const args = ['display-message', '-p'];
  if (env.TMUX_PANE) args.push('-t', env.TMUX_PANE); // the pane Claude runs in, not the active one
  args.push('#S');
  return execFileSync('tmux', args, { encoding: 'utf8', timeout: 1000, stdio: ['ignore', 'pipe', 'ignore'] }).trim();
}

function trim(file) {
  const lines = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean);
  if (lines.length <= MAX_LINES) return;
  // shortcut: two hooks trimming at the same instant can drop one line; fine for a status view.
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, lines.slice(-MAX_LINES).join('\n') + '\n');
  fs.renameSync(tmp, file);
}

function main() {
  const env = process.env;
  if (!env.TMUX) return; // not in tmux: nothing to attach the sub-agent to
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  const event = EVENTS[input.hook_event_name];
  // Empty agent_type = Claude's internal agents (prompt suggestions, /btw): not sub-agents.
  if (!event || !input.agent_id || !input.agent_type) return;
  const session = tmuxSession(env);
  if (!session) return;
  // Docs show agent_id both with and without the `agent-` prefix; the transcript file
  // is always subagents/agent-<id>.jsonl. Normalize so start and stop share one id.
  const id = String(input.agent_id).replace(/^agent-/, '');
  let transcript = expandHome(input.agent_transcript_path);
  if (!transcript && input.transcript_path && input.session_id) {
    // SubagentStart has no agent_transcript_path: derive it next to the main transcript.
    transcript = path.join(path.dirname(expandHome(input.transcript_path)), String(input.session_id), 'subagents', `agent-${id}.jsonl`);
  }
  const line = {
    ts: Math.floor(Date.now() / 1000),
    cli: 'claude',
    event,
    id,
    parent_id: input.session_id || undefined,
    name: String(input.agent_type),
    transcript: transcript || undefined,
  };
  const dir = path.join(env.XDG_STATE_HOME || path.join(os.homedir(), '.local', 'state'), 'cdamux', 'agents');
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, `${session.replace(/\//g, '_')}.jsonl`);
  fs.appendFileSync(file, JSON.stringify(line) + '\n');
  trim(file);
}

try { main(); } catch (e) { /* fail-open */ }
process.exit(0);
