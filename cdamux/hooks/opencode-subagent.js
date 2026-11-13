// cdamux — opencode plugin: appends start/stop lines for child sessions (sessions
// with a parentID, i.e. the `task` tool's sub-agents) to the sub-agent ledger of
// the tmux session opencode runs in (schema: cdamux/docs/subagents.md), and stores
// the main session id (no parentID) as agent_session_id in its restore record
// (ADR-009), and reports the main session's working / idle / error status (ADR-011 §3).
// ESM, as opencode loads plugins. Fail-open: errors are swallowed; local only.
// Enable: install.sh --cdamux-hooks links it into ~/.config/opencode/plugins/.
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// opencode loads this through a symlink: resolve the real path to find ledger.js.
const realPath = fs.realpathSync(fileURLToPath(import.meta.url));
const { append, setAgentSessionId, setAgentStatus } = createRequire(realPath)(path.join(path.dirname(realPath), 'ledger.js'));

export const CdamuxSubagent = async () => {
  const children = new Map(); // child session id -> parent session id
  let mainId = null;
  const add = (info) => {
    if (info && info.id && !info.parentID) {
      mainId = info.id;
      setAgentSessionId(process.env, info.id, process.pid); // in-process: start at opencode itself
      setAgentStatus(process.env, 'idle');
      return;
    }
    if (!info || !info.parentID || children.has(info.id)) return;
    children.set(info.id, info.parentID);
    append(process.env, { cli: 'opencode', event: 'start', id: info.id, parent_id: info.parentID, name: info.title || undefined });
  };
  const stop = (id, status) => {
    if (id && id === mainId) {
      setAgentStatus(process.env, status === 'error' ? 'error' : 'idle');
      return;
    }
    if (!children.has(id)) return;
    append(process.env, { cli: 'opencode', event: 'stop', id, parent_id: children.get(id), status });
    children.delete(id);
  };
  return {
    event: async ({ event }) => {
      try {
        const p = event.properties || {};
        // shortcut: a child resumed after going idle is not shown again; revisit if opencode reuses child sessions.
        if (event.type === 'session.created') add(p.info);
        // A reported `idle` beats the pane heuristic, so busy must be reported too (stickiness: ledger.js).
        else if (event.type === 'session.status' && p.status && p.status.type === 'busy' && p.sessionID === mainId) setAgentStatus(process.env, 'working');
        else if (event.type === 'session.idle') stop(p.sessionID);
        else if (event.type === 'session.error') stop(p.sessionID, 'error');
        else if (event.type === 'session.deleted') stop(p.info && p.info.id);
      } catch (e) { /* fail-open */ }
    },
  };
};
