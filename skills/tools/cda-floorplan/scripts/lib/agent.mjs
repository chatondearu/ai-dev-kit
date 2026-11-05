// Which coding agent is running the skill. Recorded on the map so Ask / "Fix with" name the right one.
// Detection order matches orch.sh: agent, cursor-agent, claude, opencode — plus codex.
// --agent / --engine (or FLOORPLAN_AGENT / FLOORPLAN_ENGINE) override; otherwise PATH + env hints.
import { execFileSync } from 'node:child_process';

export const AGENTS = ['agent', 'cursor-agent', 'claude', 'opencode', 'codex'];
/** Preferred discovery order (orch.sh), then codex. */
export const DETECT_ORDER = ['agent', 'cursor-agent', 'claude', 'opencode', 'codex'];

function onPath(cmd, env = process.env) {
  try {
    const which = process.platform === 'win32' ? 'where' : 'which';
    execFileSync(which, [cmd], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
      env,
    });
    return true;
  } catch {
    return false;
  }
}

export function detectAgent(args = {}, env = process.env) {
  const v = String(args.agent || args.engine || env.FLOORPLAN_AGENT || env.FLOORPLAN_ENGINE || '').toLowerCase();
  if (AGENTS.includes(v)) return v;
  if (env.CODEX_THREAD_ID || env.CODEX_SESSION_ID) return 'codex';
  for (const name of DETECT_ORDER) {
    if (onPath(name, env)) return name;
  }
  return 'claude';
}
