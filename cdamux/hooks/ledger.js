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

const stateDir = (env, sub) => path.join(
  env.CDAMUX_HOME || path.join(env.XDG_STATE_HOME || path.join(os.homedir(), '.local', 'state'), 'cdamux'),
  sub,
);

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

// Agent id for status writes: env first, else tmux `@cdamux_id` of this pane.
// Reject empty / placeholder expansions (tmux prints the format literally when unset).
function resolveAgentId(env) {
  const fromEnv = (env.CDAMUX_AGENT_ID || '').trim();
  if (fromEnv) return fromEnv;
  if (!env.TMUX) return null;
  try {
    const id = tmuxDisplay(env, '#{@cdamux_id}').trim();
    if (!id || id === '#{@cdamux_id}' || id.includes('/')) return null;
    return id;
  } catch (e) {
    return null;
  }
}

function readAgentStatus(env, id) {
  if (!id) return null;
  try {
    const file = path.join(stateDir(env, 'agents'), `${id}.json`);
    const rec = JSON.parse(fs.readFileSync(file, 'utf8'));
    const s = rec && typeof rec.status === 'string' ? rec.status.trim().toLowerCase() : '';
    return s || null;
  } catch (e) {
    return null;
  }
}

function stickyOk(current, next) {
  // A pending ask (`blocked`) must survive the asker's own turn-end hook.
  if (current === 'done' || current === 'error' || current === 'blocked') return next === 'working';
  return true;
}

/// Write status via `cdamux state append` (ADR-011 single write path). No-op without
/// agent id or without `cdamux` on PATH. Fail-open.
function setAgentStatus(env, status) {
  const id = resolveAgentId(env);
  if (!id || !status) return;
  const current = readAgentStatus(env, id);
  if (!stickyOk(current, status)) return;
  try {
    execFileSync('cdamux', ['state', 'append', 'agents', id, JSON.stringify({ status })], {
      env,
      timeout: 40,
      stdio: 'ignore',
    });
  } catch (e) {
    /* fail-open */
  }
}

function readCursor(file) {
  try {
    const n = parseInt(fs.readFileSync(file, 'utf8').trim(), 10);
    return Number.isFinite(n) && n >= 0 ? n : 0;
  } catch (e) {
    return 0;
  }
}

/// Unread inbox lines after the ack cursor (direct read — hooks stay fast / fail-open).
function readUnreadMessages(env, id) {
  if (!id) return [];
  try {
    const dir = stateDir(env, 'inbox');
    const jsonl = path.join(dir, `${id}.jsonl`);
    const cursor = readCursor(path.join(dir, `${id}.cursor`));
    const fd = fs.openSync(jsonl, 'r');
    try {
      const len = fs.fstatSync(fd).size;
      if (cursor >= len) return [];
      const buf = Buffer.alloc(len - cursor);
      fs.readSync(fd, buf, 0, buf.length, cursor);
      return buf.toString('utf8').split('\n').filter(Boolean).map((line) => {
        try { return JSON.parse(line); } catch (e) { return null; }
      }).filter(Boolean);
    } finally {
      fs.closeSync(fd);
    }
  } catch (e) {
    return [];
  }
}

/// One-line-ish digest for turn-end / prompt-submit hooks. Empty when nothing unread.
function unreadDigest(env) {
  const id = resolveAgentId(env);
  const msgs = readUnreadMessages(env, id);
  if (!msgs.length) return '';
  const lines = [`cdamux: ${msgs.length} unread message(s) — run: cdamux msg read --ack`];
  for (const m of msgs.slice(0, 5)) {
    const text = String(m.text || '').slice(0, 80);
    lines.push(`- [${m.kind || '?'}] from ${m.from || '?'}: ${text}`);
  }
  if (msgs.length > 5) lines.push(`- …and ${msgs.length - 5} more`);
  return lines.join('\n');
}

module.exports = {
  append,
  expandHome,
  setAgentSessionId,
  resolveAgentId,
  readAgentStatus,
  setAgentStatus,
  readUnreadMessages,
  unreadDigest,
  stateDir,
};
