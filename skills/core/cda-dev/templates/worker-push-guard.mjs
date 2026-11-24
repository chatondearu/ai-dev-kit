#!/usr/bin/env node
// Worker push guard (ADR-019): Claude Code PreToolUse hook on Bash, wired by the rendered
// worker --settings file as `node worker-push-guard.mjs <own> <default> <worktree> <kit>`.
// Strict whitelist, no shell parsing (owner decision 2026-11-24: every tokenizer had
// bypasses). A command whose text mentions push / send-pack / receive-pack (quotes and
// backslashes removed, any case) runs only as one plain line without shell metacharacters:
//   git push [-u|--set-upstream] [--force-with-lease[=<own>[:<40-hex sha>]]] origin <own>
//   (or origin HEAD:refs/heads/<own>), or bare `git push` / `git push origin` on the own
//   upstream; every form only with no push-redirecting git config (pushConfigReason).
// Shell script runs (`bash|sh <relative path in the worktree> [args]`, `./<path>`) follow
// the same plain-line rule; read-only kit scripts run only in their listed forms. Git with
// $, backticks, xargs or eval, git alias / config-env / include, and `gh` merge or ref
// updates are refused. Exit 2 with the reason on stderr.
// Text is not a sandbox: a script the worker writes can still push; branch protection is
// the real guard.
import { execFileSync } from 'node:child_process';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import { resolve, sep } from 'node:path';

const [own = '', dflt = '', worktree = '', kit = ''] = process.argv.slice(2);
let root = '';
try {
  root = worktree && realpathSync(worktree);
} catch {
  root = ''; // unknown worktree: no script may run
}

// Any of these makes a line more than one plain command.
const META = /[$`'"\\;&|<>(){}#\n\r\t]/;
const PUSH_FORM = `git push [-u] [--force-with-lease[=${own}[:<sha>]]] `
  + `origin ${own}|HEAD:refs/heads/${own}`;

const gitIn = (cwd, ...a) => {
  try {
    return execFileSync('git', a, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return '';
  }
};

// Git config that can redirect any push (named or bare) and that the guard does not model:
// any `push.*` but `push.default` simple/current/upstream, `remote.pushDefault`,
// `remote.*.push*` (refmap, pushurl), `remote.*.mirror`, `branch.*.pushRemote`,
// `url.*.insteadOf` / `pushInsteadOf`. Read in the hook's cwd (repo, global, system).
const PUSH_CONFIG = '^(push\\.|remote\\.pushdefault$|remote\\..*\\.(push|mirror)'
  + '|branch\\..*\\.pushremote$|url\\..*\\.(push)?insteadof$)';
function pushConfigReason(cwd) {
  const cfg = gitIn(cwd, 'config', '--get-regexp', PUSH_CONFIG);
  const modeled = /^push\.default (simple|current|upstream)$/i;
  const bad = cfg && cfg.split('\n').find((l) => !modeled.test(l.trim()));
  return bad ? `git config ${bad.split(' ')[0]} can redirect a push; the guard refuses it` : null;
}

// Bare `git push [origin]` lets git pick what to push: only from the hook's cwd on the own
// branch with upstream `refs/heads/<own>`.
function implicitPushReason(cwd) {
  const hint = `name the branch: git push origin ${own}`;
  if (gitIn(cwd, 'symbolic-ref', '--short', '-q', 'HEAD') !== own) return hint;
  if (gitIn(cwd, 'config', `branch.${own}.merge`) !== `refs/heads/${own}`) return hint;
  return null;
}

// Reason a line that mentions a push is refused, or null.
function pushReason(t, cwd) {
  if (!own || own === dflt || /^([+-]|(refs|heads|tags|remotes)\/)/.test(own)) {
    return 'this worker may not push (no own branch)';
  }
  if (META.test(t)) return `a push must be one plain line: ${PUSH_FORM}`;
  const [git, sub, ...rest] = t.split(/ +/);
  if (git !== 'git' || sub !== 'push') return `a push must be one plain line: ${PUSH_FORM}`;
  const cfg = pushConfigReason(cwd);
  if (cfg) return cfg;
  if (rest.length === 0 || (rest.length === 1 && rest[0] === 'origin')) return implicitPushReason(cwd);
  const seen = new Set();
  const pos = [];
  for (const a of rest) {
    let flag = null;
    if (a === '-u' || a === '--set-upstream') {
      flag = 'u';
    } else if (a === '--force-with-lease') {
      flag = 'lease';
    } else if (a.startsWith('--force-with-lease=')) {
      const [ref, sha, ...more] = a.slice('--force-with-lease='.length).split(':');
      if (ref !== own || more.length || (sha !== undefined && !/^[0-9a-f]{40}$/.test(sha))) {
        return `lease ${a} is not allowed: --force-with-lease[=${own}[:<40-hex sha>]]`;
      }
      flag = 'lease';
    } else if (a.startsWith('-')) {
      return `git push ${a} is not allowed: ${PUSH_FORM}`;
    } else {
      pos.push(a);
    }
    if (flag && seen.has(flag)) return `git push ${a} repeated`;
    if (flag) seen.add(flag);
  }
  const dst = pos[1] === `HEAD:refs/heads/${own}` ? own : pos[1];
  if (pos.length !== 2 || pos[0] !== 'origin' || dst !== own) {
    return `push only to the own branch: ${PUSH_FORM}`;
  }
  return null;
}

// Read-only kit scripts a worker may run by absolute path, with exactly these arguments
// (doc-hygiene may add the worktree path).
const KIT_FORMS = [
  ['skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh', '--check'],
  ['skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh', '--report'],
  ['skills/core/cda-kanban/scripts/foam-board.sh', 'list-ready'],
  ['skills/core/cda-kanban/scripts/foam-board.sh', 'list-ready', '--json'],
  ['skills/core/cda-kanban/scripts/foam-board.sh', 'reconcile'],
];
const kitForm = (args) => Boolean(kit) && KIT_FORMS.some(([p, ...a]) => args[0] === `${kit}/${p}`
  && [a, p.endsWith('doc-hygiene.sh') ? [...a, worktree] : null]
    .some((f) => f && f.join(' ') === args.slice(1).join(' ')));

// Reason a line that runs a shell or `./x` is refused, or null: one plain line
// `[nix develop -c] bash|sh <relative path> [args]` or `./<path> [args]`, the path a file
// inside the worktree (symlinks resolved, no `..`), or a KIT_FORMS line.
function scriptReason(t, cwd) {
  const form = 'run a script as one plain line: bash|sh <path in the worktree> [args]';
  if (META.test(t)) return form;
  let w = t.split(/ +/);
  if (w.slice(0, 3).join(' ') === 'nix develop -c') w = w.slice(3);
  const viaShell = w[0] === 'bash' || w[0] === 'sh';
  if (!viaShell && !(w[0] ?? '').startsWith('./')) return form;
  const args = viaShell ? w.slice(1) : w;
  if (viaShell && kitForm(args)) return null;
  const p = args[0] ?? '';
  if (!p || p.startsWith('-') || p.startsWith('/')) return form;
  if (p.split('/').includes('..')) return `script path ${p} contains ..`;
  let real;
  try {
    real = realpathSync(resolve(cwd, p));
    if (!statSync(real).isFile()) throw new Error('not a file');
  } catch {
    return `script ${p} is not a file`;
  }
  const inside = root && (real === root || real.startsWith(root + sep));
  return inside ? null : `script ${p} is outside the worktree`;
}

// Reason the command text is refused, or null.
function reason(cmd, cwd) {
  const t = cmd.trim();
  const flat = t.replace(/['"\\]/g, ''); // pu''sh, p"u"sh, pu\sh
  const git = /\bgit\b/i.test(flat);
  if (git && /alias\.|--config-env|include\.?path|includeif/i.test(flat)) {
    return 'git aliases, --config-env and config includes are not allowed';
  }
  if (/push|send-pack|receive-pack/i.test(flat)) return pushReason(t, cwd);
  if (git && (/[$`]/.test(t) || /\b(xargs|eval)\b/.test(flat))) {
    return 'git with $, backticks, xargs or eval cannot be checked';
  }
  if (/\bgh\b/.test(flat) && /merge|git\/refs|updateref|deleteref|createref/i.test(flat)) {
    return 'gh merge / ref updates are not allowed';
  }
  if (/(^|[^\w.-])(bash|sh|zsh|dash|ksh)\b/.test(flat) || /(^|[;&|(){}\n`]\s*)\.\.?\//.test(t)) {
    return scriptReason(t, cwd);
  }
  return null;
}

let input;
try {
  input = JSON.parse(readFileSync(0, 'utf8'));
} catch {
  process.exit(0); // no hook payload: nothing to judge
}
const cmd = input?.tool_name === 'Bash' ? String(input?.tool_input?.command ?? '') : '';
const r = cmd && reason(cmd, String(input?.cwd || process.cwd()));
if (r) {
  process.stderr.write(`worker push guard: ${r} (ADR-019). A denied command is an escalation `
    + 'to the maintainer through the orchestrator; never work around it.\n');
  process.exit(2);
}
