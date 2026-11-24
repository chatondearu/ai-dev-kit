#!/usr/bin/env node
// Worker push guard (ADR-019): Claude Code PreToolUse hook on Bash, wired by the rendered
// worker --settings file as `node worker-push-guard.mjs <own> <default> <worktree> <kit>`.
// Strict whitelist, no shell parsing (owner decision 2026-11-24: every tokenizer had
// bypasses). A command whose text mentions push / send-pack / receive-pack (quotes and
// backslashes removed, any case) runs only as one plain line without shell metacharacters:
//   git push [-u|--set-upstream] [--force-with-lease[=refs/heads/<own>[:<40-hex sha>]]]
//     origin HEAD:refs/heads/<own>
//   only (an explicit destination: no refmap, symref or upstream picks it), and only with
//   no push-redirecting git state (pushStateReason, fail closed).
// Repo tooling with a shell, node or a command runner in it (`bash|sh|./|scripts/<file>`,
// `node <file>`, `mise exec --` / `nix develop -c` + cargo, make, just, npm|pnpm run …)
// follows the same plain-line rule, files inside the worktree; read-only kit scripts run
// only in their listed forms. Git with $, backticks, xargs or eval, git alias / config-env
// / include, and `gh` merge or ref updates are refused. Exit 2 with the reason on stderr.
// Text is not a sandbox: a script the worker writes can still push; branch protection is
// the real guard.
import { spawnSync } from 'node:child_process';
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
const DST = `HEAD:refs/heads/${own}`;
const LEASE = `--force-with-lease[=refs/heads/${own}[:<40-hex sha>]]`;
const PUSH_FORM = `git push [-u] [${LEASE}] origin ${DST}`;

// Run git in `cwd`: { status, out }; status -1 when git cannot run or its output overflows.
const gitRun = (cwd, ...a) => {
  const opts = { cwd, encoding: 'utf8', maxBuffer: 64 << 20, stdio: ['ignore', 'pipe', 'ignore'] };
  const r = spawnSync('git', a, opts);
  return { status: r.error ? -1 : r.status, out: (r.stdout ?? '').trim() };
};

// Git state that can still redirect an explicit `HEAD:refs/heads/<own>` push, or that the
// guard does not model: any `push.*` but `push.default` simple/current/upstream,
// `remote.pushDefault`, `remote.*.push*` (refmap, pushurl), `remote.*.mirror`,
// `branch.*.pushRemote`, `url.*.insteadOf` / `pushInsteadOf`, `submodule.recurse`, and a
// symbolic `refs/heads/<own>`. Fails closed: outside a git worktree, or when git config
// cannot be read in full (status other than 0 or 1), the push is refused.
const PUSH_CONFIG = '^(push\\.|remote\\.pushdefault$|remote\\..*\\.(push|mirror)'
  + '|branch\\..*\\.pushremote$|url\\..*\\.(push)?insteadof$|submodule\\.recurse$)';
function pushStateReason(cwd) {
  if (gitRun(cwd, 'rev-parse', '--is-inside-work-tree').out !== 'true') {
    return 'push from inside the worktree';
  }
  const { status, out } = gitRun(cwd, 'config', '--get-regexp', PUSH_CONFIG);
  if (status !== 0 && status !== 1) return 'git config could not be read in full; push refused';
  const modeled = /^push\.default (simple|current|upstream)$/i;
  const bad = out && out.split('\n').find((l) => !modeled.test(l.trim()));
  if (bad) return `git config ${bad.split(' ')[0]} can redirect a push; push refused`;
  if (gitRun(cwd, 'symbolic-ref', '-q', `refs/heads/${own}`).status !== 1) {
    return `refs/heads/${own} is a symbolic ref (or unreadable); push refused`;
  }
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
      const badSha = sha !== undefined && !/^[0-9a-f]{40}$/.test(sha);
      if (ref !== `refs/heads/${own}` || more.length || badSha) {
        return `lease ${a} is not allowed: ${LEASE}`;
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
  // Only an explicit refs/heads destination: refmaps and symrefs cannot redirect it.
  if (pos.length !== 2 || pos[0] !== 'origin' || pos[1] !== DST) {
    return `push only with an explicit destination: ${PUSH_FORM}`;
  }
  return pushStateReason(cwd);
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

const TOOL_FORM = 'run repo tooling as one plain line, no pipe/redirect/$/quotes: cargo …, '
  + 'make|just …, npm|pnpm run …, node <file>, bash|sh|./|scripts/<script> …, optionally '
  + 'behind mise exec -- or nix develop -c';

// Reason `p` is not a file inside the worktree (relative, symlinks resolved, no `..`).
function pathReason(p, cwd) {
  if (!p || p.startsWith('-') || p.startsWith('/')) {
    return `name a file of the worktree by relative path: ${p}`;
  }
  if (p.split('/').includes('..')) return `path ${p} contains ..`;
  let real;
  try {
    real = realpathSync(resolve(cwd, p));
    if (!statSync(real).isFile()) throw new Error('not a file');
  } catch {
    return `${p} is not a file`;
  }
  const inside = root && (real === root || real.startsWith(root + sep));
  return inside ? null : `${p} is outside the worktree`;
}

// npm / pnpm subcommands a worker may run (no exec/dlx: they run any package).
const NPM_SUBS = {
  npm: ['run', 'test', 'ci', 'install'],
  pnpm: ['install', 'run', 'test', 'build', 'lint', 'typecheck'],
};

// Reason the words of one plain line are not a repo tool run, or null. `mise exec --` and
// `nix develop -c|--command` may wrap one tool (not each other): they run anything else.
function toolReason(w, cwd, wrapped = false) {
  const [cmd = '', ...args] = w;
  if (!wrapped && cmd === 'mise' && args[0] === 'exec' && args[1] === '--') {
    return toolReason(args.slice(2), cwd, true);
  }
  const nixDevelop = cmd === 'nix' && args[0] === 'develop' && ['-c', '--command'].includes(args[1]);
  if (!wrapped && nixDevelop) {
    return toolReason(args.slice(2), cwd, true);
  }
  if (cmd === 'cargo') return args.includes('install') ? 'cargo install is not allowed' : null;
  if (cmd === 'make' || cmd === 'just') return null;
  if (Object.hasOwn(NPM_SUBS, cmd)) {
    const global = args.some((a) => /^(-g$|--global|--prefix|--location)/.test(a));
    const ok = NPM_SUBS[cmd].includes(args[0]) && !global;
    return ok ? null : `${cmd} ${args[0] ?? ''} is not allowed`;
  }
  if (cmd === 'node') return pathReason(args[0], cwd);
  if (cmd === 'bash' || cmd === 'sh') return kitForm(args) ? null : pathReason(args[0], cwd);
  if (cmd.startsWith('./') || cmd.startsWith('scripts/')) return pathReason(cmd, cwd);
  return TOOL_FORM;
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
  // A shell, a runner or node anywhere, or a `./` / `scripts/` path at a command start.
  if (/(^|[^\w.-])(bash|sh|zsh|dash|ksh|node|mise|nix)\b/.test(flat)
    || /(^|[;&|(){}\n`]\s*)(\.\.?\/|scripts\/)/.test(t)) {
    return META.test(t) ? TOOL_FORM : toolReason(t.split(/ +/), cwd);
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
