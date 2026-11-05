// cdamux — shared writers for every CLI adapter: the sub-agent ledger (schema:
// cdamux/docs/subagents.md) and the agent session id in the restore registry
// (ADR-009). Local files only; callers wrap them in fail-open try/catch.
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');

const MAX_LINES = 500;

const expandHome = (p) => (p && p.startsWith('~/') ? path.join(os.homedir(), p.slice(2)) : p);

const run = (cmd, args) => execFileSync(cmd, args, { encoding: 'utf8', timeout: 1000, stdio: ['ignore', 'pipe', 'ignore'] }).trim();

function tmuxDisplay(env, format) {
  const args = ['display-message', '-p'];
  if (env.TMUX_PANE) args.push('-t', env.TMUX_PANE); // the pane the CLI runs in, not the active one
  return run('tmux', [...args, format]);
}

const tmuxSession = (env) => tmuxDisplay(env, '#S');

const SHELLS = new Set(['sh', 'bash', 'zsh', 'fish', 'dash', 'ksh', 'mksh', 'tcsh', 'csh', 'nu']);
const MAX_HOPS = 8;

// True when `pid` (the CLI itself, or the hook it spawned) belongs to the pane's top-level
// agent: on the way up to the pane's pid, at most one process is not a shell (the CLI;
// pane, login and `sh -c` hook shells are skipped). A nested CLI (`claude -p` or orch.sh
// from a tool call) also passes the top CLI on the way, so it sees two. ps errors throw:
// the callers' fail-open then writes nothing.
// shortcut: a non-shell launcher that stays alive (npm's node wrapper of opencode) also
// counts, so such an agent is skipped; add it here if one shows up.
function isPaneTopLevel(panePid, pid) {
  let others = 0;
  for (let hop = 0; hop < MAX_HOPS && pid > 1; hop++) {
    if (pid === panePid) return true;
    const m = /^\s*(\d+)\s+(.*)$/.exec(run('ps', ['-o', 'ppid=,comm=', '-p', String(pid)]));
    if (!m) return false;
    if (!SHELLS.has(path.basename(m[2]).replace(/^-/, '')) && ++others > 1) return false;
    pid = Number(m[1]);
  }
  return false;
}

const stateDir = (env, sub) => path.join(env.XDG_STATE_HOME || path.join(os.homedir(), '.local', 'state'), 'cdamux', sub);

function trim(file) {
  const lines = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean);
  if (lines.length <= MAX_LINES) return;
  // shortcut: two hooks trimming at the same instant can drop one line; fine for a status view.
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, lines.slice(-MAX_LINES).join('\n') + '\n');
  fs.renameSync(tmp, file);
}

// Append {cli, event, id, ...} (ts is added) to the ledger of the tmux session `env` runs in.
// Not in tmux: nothing to attach the sub-agent to, nothing written.
function append(env, line) {
  if (!env.TMUX) return;
  const session = tmuxSession(env);
  if (!session) return;
  const dir = stateDir(env, 'agents');
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, `${session.replace(/\//g, '_')}.jsonl`);
  fs.appendFileSync(file, JSON.stringify({ ts: Math.floor(Date.now() / 1000), ...line }) + '\n');
  trim(file);
}

// Set agent_session_id (and updated_ts) in the restore record of the tmux session `env`
// runs in. Only an existing record is updated (cdamux-spawned sessions); other keys are kept.
// Only the pane's top-level CLI may set it (a nested CLI would point restore at a worker):
// `fromPid` is the first process to check, the hook's parent by default (the CLI, or the
// `sh -c` running the hook); an in-process plugin passes its own pid.
function setAgentSessionId(env, id, fromPid = process.ppid) {
  if (!env.TMUX || !id) return;
  const out = tmuxDisplay(env, '#{pane_pid} #S');
  const sp = out.indexOf(' ');
  const panePid = parseInt(out.slice(0, sp), 10);
  const session = out.slice(sp + 1);
  if (sp < 1 || !(panePid > 1) || !session || !isPaneTopLevel(panePid, fromPid)) return;
  const file = path.join(stateDir(env, 'sessions'), `${session.replace(/\//g, '_')}.json`);
  if (!fs.existsSync(file)) return;
  const rec = JSON.parse(fs.readFileSync(file, 'utf8'));
  const sid = String(id);
  if (!rec || typeof rec !== 'object' || Array.isArray(rec) || rec.agent_session_id === sid) return;
  rec.agent_session_id = sid;
  rec.updated_ts = Math.floor(Date.now() / 1000);
  const tmp = `${file}.tmp.${process.pid}`; // same tmp + rename as the Rust writer
  try {
    fs.writeFileSync(tmp, JSON.stringify(rec, null, 2));
    fs.renameSync(tmp, file);
  } catch (e) {
    fs.rmSync(tmp, { force: true });
    throw e;
  }
}

module.exports = { append, expandHome, setAgentSessionId };
