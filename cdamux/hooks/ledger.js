// cdamux — shared writers for every CLI adapter: the sub-agent ledger (schema:
// cdamux/docs/subagents.md) and the agent session id in the restore registry
// (ADR-009). Local files only; callers wrap them in fail-open try/catch.
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');

const MAX_LINES = 500;

const expandHome = (p) => (p && p.startsWith('~/') ? path.join(os.homedir(), p.slice(2)) : p);

function tmuxSession(env) {
  const args = ['display-message', '-p'];
  if (env.TMUX_PANE) args.push('-t', env.TMUX_PANE); // the pane the CLI runs in, not the active one
  args.push('#S');
  return execFileSync('tmux', args, { encoding: 'utf8', timeout: 1000, stdio: ['ignore', 'pipe', 'ignore'] }).trim();
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
function setAgentSessionId(env, id) {
  if (!env.TMUX || !id) return;
  const session = tmuxSession(env);
  if (!session) return;
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
