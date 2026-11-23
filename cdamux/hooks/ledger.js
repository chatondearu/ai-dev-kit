// cdamux — shared writers for every CLI adapter: the sub-agent ledger (schema:
// cdamux/docs/subagents.md) and the agent session id in the restore registry
// (ADR-009). Local files only; callers wrap them in fail-open try/catch.
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync, spawn } = require('child_process');

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

// True when this hook belongs to the pane's top-level CLI, or runs outside tmux. A nested
// CLI (`claude -p` from a tool call) inherits CDAMUX_AGENT_ID and TMUX: it must not take
// the parent's inbox nor rewrite its status / turn. tmux or ps errors: false (no effect).
function isTopLevelHook(env, fromPid = process.ppid) {
  if (!env.TMUX) return true;
  try {
    const panePid = parseInt(tmuxDisplay(env, '#{pane_pid}'), 10);
    return panePid > 1 && isPaneTopLevel(panePid, fromPid);
  } catch (e) {
    return false;
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

function readAgentField(env, id, key) {
  if (!id) return null;
  try {
    const file = path.join(stateDir(env, 'agents'), `${id}.json`);
    const rec = JSON.parse(fs.readFileSync(file, 'utf8'));
    const s = rec && typeof rec[key] === 'string' ? rec[key].trim().toLowerCase() : '';
    return s || null;
  } catch (e) {
    return null;
  }
}

const readAgentStatus = (env, id) => readAgentField(env, id, 'status');

function replyTimedOut(err) {
  return !!err && (err.code === 'ETIMEDOUT' || err.killed === true || err.signal === 'SIGTERM');
}

// Turn end of an orchestrator: hand the hook payload to `cdamux events reply`, which
// records its last assistant prose — or one tool-only line when there is none
// (docs/events.md). A 1s timeout must not leave the journal empty: retry once
// without the transcript (the slow read). Fail-open either way: never fail the turn.
function recordReply(env, input) {
  const id = resolveAgentId(env);
  if (readAgentField(env, id, 'role') !== 'orchestrator') return;
  const call = (payload) => execFileSync('cdamux', ['events', 'reply', id], {
    env,
    input: JSON.stringify(payload ?? {}),
    timeout: 1000,
    stdio: ['pipe', 'ignore', 'ignore'],
  });
  try {
    call(input);
  } catch (e) {
    if (!replyTimedOut(e)) return;
    try {
      const fast = input && typeof input === 'object' ? { ...input } : {};
      delete fast.transcript_path;
      call(fast);
    } catch (e2) {
      /* fail-open */
    }
  }
}

// Turn end of an orchestrator: paste the next queued `cdamux say` prompt (docs/say.md).
// The queue file is checked first so an empty queue costs no process. Fail-open.
function drainSay(env) {
  const id = resolveAgentId(env);
  if (readAgentField(env, id, 'role') !== 'orchestrator') return;
  try {
    if (fs.statSync(path.join(stateDir(env, 'say'), `${id}.jsonl`)).size === 0) return;
    // The paste waits 300 ms between its tmux writes: leave room.
    execFileSync('cdamux', ['msg', 'drain', id], { env, timeout: 3000, stdio: 'ignore' });
  } catch (e) {
    /* fail-open */
  }
}

function stickyOk(current, next) {
  // A pending ask (`blocked`) must survive the asker's own turn-end hook.
  if (current === 'done' || current === 'error' || current === 'blocked') return next === 'working';
  return true;
}

/// Status only (opencode plugin): same write path as [`setAgentState`].
function setAgentStatus(env, status) {
  if (status) setAgentState(env, status);
}

/// Fire-and-forget `cdamux state tick` (blocked-worker reminder, throttled in cdamux):
/// hooks are the only periodic signal while no Fleet is open. Never waits, fail-open.
/// An older binary without `state tick` just exits 2.
function kickTick(env) {
  try {
    spawn('cdamux', ['state', 'tick'], { env, detached: true, stdio: 'ignore' })
      .on('error', () => {})
      .unref();
  } catch (e) {
    /* fail-open */
  }
}

/// One `state append` with `status` (when stickiness allows it) and `turn` (`running` /
/// `ended` / `exited`, never sticky: msg wake reads it to know a pane is safe to type
/// into, ADR-018). `--sticky` re-checks stickiness under the record lock (it drops only
/// `status`, so `turn` still lands); an older binary without it exits 2 and gets the
/// plain append. 2.5 s budget (hooks get 5 s): a dropped `turn` write leaves a stale
/// value. Fail-open.
function setAgentState(env, status, turn) {
  const id = resolveAgentId(env);
  if (!id) return;
  const patch = {};
  if (status && stickyOk(readAgentStatus(env, id), status)) patch.status = status;
  if (turn) patch.turn = turn;
  if (!Object.keys(patch).length) return;
  kickTick(env);
  const args = ['state', 'append', 'agents', id, JSON.stringify(patch)];
  const opts = { env, timeout: 2500, stdio: 'ignore' };
  try {
    execFileSync('cdamux', [...args, '--sticky'], opts);
  } catch (e) {
    if (e && e.status === 2) {
      try {
        execFileSync('cdamux', args, opts);
      } catch (e2) {
        /* fail-open */
      }
    }
  }
}

// Consecutive Stop blocks per turn: past it the stop goes through and the rest waits for
// the wake line, so a peer sending faster than turns end cannot keep an agent running.
const MAX_STOP_BLOCKS = 3;

/// May this Stop block? `continued` (stop_hook_active): the stop follows a block, so count
/// it; a fresh stop resets. Counter: `inbox/<id>.stopblocks`. Fail-open to "may block".
function takeStopBlock(env, continued) {
  const id = resolveAgentId(env);
  if (!id) return false;
  const file = path.join(stateDir(env, 'inbox'), `${id}.stopblocks`);
  let n = 0;
  try {
    if (continued) n = parseInt(fs.readFileSync(file, 'utf8'), 10) || 0;
  } catch (e) {
    n = 0;
  }
  if (n >= MAX_STOP_BLOCKS) return false;
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, String(n + 1));
  } catch (e) {
    /* fail-open */
  }
  return true;
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

// Inbox JSON handed over per hook call: well under Claude Code's hook output cap; the rest
// stays unread for the next Stop / prompt.
const HANDOVER_MAX_BYTES = 8000;

/// Read and ack unread lines (up to HANDOVER_MAX_BYTES) through `cdamux msg read --ack`
/// (Rust owns the cursor, acks only what it prints). Empty on any failure, so a hook never
/// hands over what it could not ack.
function takeUnread(env) {
  const id = resolveAgentId(env);
  if (!id || !readUnreadMessages(env, id).length) return [];
  try {
    const args = ['msg', 'read', '--ack', '--agent', id, '--max-bytes', String(HANDOVER_MAX_BYTES)];
    const out = execFileSync('cdamux', args, {
      env,
      encoding: 'utf8',
      timeout: 2000,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
    return out.split('\n').filter(Boolean).map((line) => {
      try { return JSON.parse(line); } catch (e) { return null; }
    }).filter(Boolean);
  } catch (e) {
    return [];
  }
}

/// Full message bodies for a Stop `reason` / UserPromptSubmit context, oldest first.
function formatMessages(msgs) {
  const n = msgs.length === 1 ? '1 message' : `${msgs.length} messages`;
  const lines = [`cdamux: ${n} from your inbox (already acked; act on them):`];
  for (const m of msgs) {
    const ref = m.ref ? ` · ref ${m.ref}` : '';
    lines.push('', `[${m.kind || '?'} from ${m.from || '?'} · ${m.id || '?'}${ref}]`, String(m.text || ''));
  }
  return lines.join('\n');
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
  recordReply,
  drainSay,
  formatMessages,
  isTopLevelHook,
  kickTick,
  readAgentField,
  takeStopBlock,
  setAgentState,
  setAgentStatus,
  readUnreadMessages,
  takeUnread,
  unreadDigest,
  stateDir,
};
