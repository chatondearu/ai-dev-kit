#!/usr/bin/env node
// Worker push guard (ADR-019): Claude Code PreToolUse hook on Bash, wired by the rendered
// worker --settings file as `node worker-push-guard.mjs <own-branch> <default-branch>`.
// Bash permission rules match command text; this parses argv so spellings the rules miss
// (`-uf`, `:ref`, quoted refs, `git -C x push`, `bash -c "…"`) are still refused.
// Blocks (exit 2, reason on stderr): any `git push` / `send-pack` that does not update only
// the own branch (no plain force, delete, mirror/all/tags, no default branch; a
// `--force-with-lease` on the own branch is fine, for a rebased branch), `git -c alias.*`,
// `gh pr merge` and `gh api` merge / git-refs paths. Conservative: an unquoted `git` word
// anywhere in a command is treated as a call.
// Text is not a sandbox: a script the worker writes can still push; branch protection is
// the real guard.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const [own = '', dflt = ''] = process.argv.slice(2);

// Text of the `$(…)` starting at text[i] ('$'), and the index of its closing paren.
function substitution(text, i) {
  let depth = 0;
  for (let j = i + 1; j < text.length; j++) {
    if (text[j] === '(') depth++;
    else if (text[j] === ')' && --depth === 0) return [text.slice(i + 2, j), j];
  }
  return [text.slice(i + 2), text.length];
}

// Push the `$(…)` / backtick substitutions of `body` (data otherwise) to `subs`.
function bodySubs(body, subs) {
  for (let j = 0; j < body.length; j++) {
    if (body[j] === '\\') {
      j++;
    } else if (body[j] === '$' && body[j + 1] === '(') {
      const [sub, k] = substitution(body, j);
      subs.push(sub);
      j = k;
    } else if (body[j] === '`') {
      const k = body.indexOf('`', j + 1);
      const end = k < 0 ? body.length : k;
      subs.push(body.slice(j + 1, end));
      j = end;
    }
  }
}

// Heredoc delimiter word starting at text[i]: [word without quotes, quoted?, last index].
function delimiter(text, i) {
  let d = '';
  let quoted = false;
  for (; i < text.length && !/[\s;&|()<>]/.test(text[i]); i++) {
    if (text[i] === "'" || text[i] === '"') {
      const j = text.indexOf(text[i], i + 1);
      const end = j < 0 ? text.length : j;
      d += text.slice(i + 1, end);
      quoted = true;
      i = end;
    } else if (text[i] === '\\' && i + 1 < text.length) {
      d += text[++i];
      quoted = true;
    } else {
      d += text[i];
    }
  }
  return [d, quoted, i - 1];
}

// Split shell text into simple commands of words. Quote-aware; operators, parens, braces
// and unquoted `$(` / backticks separate commands; `#` after a blank or operator (not
// `${#`) comments to the end of the line. `$(…)` and backticks inside double quotes or
// an unquoted-delimiter heredoc body run too: their text goes to `subs` to be scanned as
// commands. Other quoted text (commit messages, PR bodies, grep patterns) and heredoc
// bodies are data. Only a `<<` the tokenizer reaches outside quotes and comments starts
// a heredoc; one with no closing line sets `unclosed` (the caller refuses the command).
function commands(text) {
  const cmds = [];
  const subs = [];
  const pending = [];
  let unclosed = null;
  let words = [];
  let w = null;
  const flushWord = () => { if (w !== null) words.push(w); w = null; };
  const flushCmd = () => { flushWord(); if (words.length) cmds.push(words); words = []; };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '#' && w === null && (i === 0 || /[\s;&|()]/.test(text[i - 1]))) {
      while (i + 1 < text.length && text[i + 1] !== '\n') i++;
    } else if (c === '<' && text[i + 1] === '<' && text[i + 2] !== '<') {
      flushWord();
      let j = i + 2;
      const dash = text[j] === '-';
      if (dash) j++;
      while (text[j] === ' ' || text[j] === '\t') j++;
      const [delim, quoted, end] = delimiter(text, j);
      pending.push({ delim, quoted, dash });
      i = end;
    } else if (c === '\n' && pending.length) {
      flushCmd();
      for (const h of pending.splice(0)) {
        const body = [];
        let found = false;
        while (!found && i < text.length) {
          let e = text.indexOf('\n', i + 1);
          if (e < 0) e = text.length;
          const line = text.slice(i + 1, e);
          i = e;
          if (h.delim && (h.dash ? line.replace(/^\t+/, '') : line) === h.delim) found = true;
          else body.push(line);
        }
        if (!found) unclosed ??= h.delim || '(empty)';
        else if (!h.quoted) bodySubs(body.join('\n'), subs);
      }
    } else if (c === "'") {
      const j = text.indexOf("'", i + 1);
      const end = j < 0 ? text.length : j;
      w = (w ?? '') + text.slice(i + 1, end);
      i = end;
    } else if (c === '"') {
      let q = '';
      let j = i + 1;
      for (; j < text.length && text[j] !== '"'; j++) {
        if (text[j] === '\\' && j + 1 < text.length) {
          q += text[++j];
        } else if (text[j] === '$' && text[j + 1] === '(') {
          const [sub, k] = substitution(text, j);
          subs.push(sub);
          q += text.slice(j, k + 1);
          j = k;
        } else if (text[j] === '`') {
          const k = text.indexOf('`', j + 1);
          const end = k < 0 ? text.length : k;
          subs.push(text.slice(j + 1, end));
          j = end;
        } else {
          q += text[j];
        }
      }
      w = (w ?? '') + q;
      i = j;
    } else if (c === '\\' && i + 1 < text.length) {
      w = (w ?? '') + text[++i];
    } else if (/\s/.test(c) && c !== '\n') {
      flushWord();
    } else if (';&|\n(){}`'.includes(c) || (c === '$' && text[i + 1] === '(')) {
      flushCmd();
      if (c === '$') i++;
    } else {
      w = (w ?? '') + c;
    }
  }
  flushCmd();
  if (pending.length) unclosed ??= pending[0].delim || '(empty)';
  return { cmds, subs, unclosed };
}

const base = (word) => word.split('/').pop();
const OPT_WITH_VALUE = new Set(['-C', '-c', '--git-dir', '--work-tree', '--namespace', '--config-env']);
const PUSH_OK_LONG = new Set(['--set-upstream', '--dry-run', '--quiet', '--verbose', '--progress',
  '--no-progress', '--no-verify', '--atomic', '--porcelain']);

const short = (ref) => ref.replace(/^refs\/heads\//, '');

// `git push [<remote>]` lets git pick what to push. Allowed only when that is provably the
// own branch alone: run where the hook runs (`moved` = the command may change directory or
// git config), on the own branch, upstream `refs/heads/<own>`, and no push config the guard
// does not model (any `push.*` but `push.default` simple/current/upstream, any
// `remote.*.push*` / `remote.*.mirror`, `url.*.pushInsteadOf`). Otherwise name the branch.
function implicitPushReason(ctx) {
  const hint = `name the branch: git push origin ${own}`;
  if (ctx.moved) return hint;
  const git = (...a) => {
    try {
      return execFileSync('git', a, { cwd: ctx.cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    } catch {
      return '';
    }
  };
  if (git('symbolic-ref', '--short', '-q', 'HEAD') !== own) return hint;
  if (git('config', `branch.${own}.merge`) !== `refs/heads/${own}`) return hint;
  const cfg = git('config', '--get-regexp', '^(push\\.|remote\\..*\\.(push|mirror)|url\\..*\\.pushinsteadof$)');
  const modeled = /^push\.default (simple|current|upstream)$/i;
  if (cfg && cfg.split('\n').some((l) => !modeled.test(l.trim()))) return hint;
  return null;
}

// Reason a `git` call (words after `git`) is refused, or null.
function gitReason(args, ctx) {
  let i = 0;
  for (; i < args.length && args[i].startsWith('-'); i++) {
    const [opt, val] = args[i].split(/=(.*)/s);
    const value = val ?? (OPT_WITH_VALUE.has(opt) ? args[++i] ?? '' : '');
    if (opt === '-c' && /^alias\./i.test(value)) return 'git -c alias.* is not allowed';
    if (opt === '--config-env') return 'git --config-env is not allowed';
  }
  if (args[i] !== 'push' && args[i] !== 'send-pack') return null;
  const positional = [];
  let lease = false;
  let ifIncludes = false;
  for (const a of args.slice(i + 1)) {
    if (a === '-o' || a.startsWith('--push-option')) return 'push options are not allowed';
    if (a.startsWith('--')) {
      const [opt, val] = a.split(/=(.*)/s);
      if (opt === '--force-with-lease') {
        lease = true;
        // `=<ref>[:<expect>]` must name the own branch; bare form leases what is pushed.
        if (val !== undefined && short(val.split(':')[0]) !== own) {
          return `lease ${a} is not allowed (own branch: ${own})`;
        }
      } else if (opt === '--force-if-includes') {
        ifIncludes = true;
      } else if (!PUSH_OK_LONG.has(opt)) {
        return `git push ${a} is not allowed`;
      }
    } else if (a.startsWith('-') && a.length > 1) {
      if (/[^unqv]/.test(a.slice(1))) return `git push ${a} is not allowed`;
    } else {
      positional.push(a);
    }
  }
  if (ifIncludes && !lease) return '--force-if-includes needs --force-with-lease';
  if (!own || own === dflt) return 'this worker may not push (no own branch)';
  if (positional.length < 2) {
    return args[i] === 'push' && i === 0 ? implicitPushReason(ctx) : `name the branch: git push origin ${own}`;
  }
  for (const spec of positional.slice(1)) {
    if (spec.startsWith('+')) return `force refspec ${spec} is not allowed`;
    const k = spec.indexOf(':');
    let dst = k < 0 ? spec : spec.slice(k + 1);
    if (k === 0 || dst === '') return `deleting a ref (${spec}) is not allowed`;
    dst = short(dst);
    if (dst !== own) return `push to ${dst} is not allowed (own branch: ${own})`;
  }
  return null;
}

const SHELLS = new Set(['sh', 'bash', 'zsh', 'dash', 'ksh']);
const GH_OPT_WITH_VALUE = new Set(['-R', '--repo', '--hostname']);

// Reason a `gh` call (words after `gh`) is refused, or null: `pr merge` (global flags such
// as `-R` anywhere) and `gh api` paths that merge or move refs.
function ghReason(args) {
  const pos = [];
  for (let i = 0; i < args.length; i++) {
    if (GH_OPT_WITH_VALUE.has(args[i])) i++;
    else if (!args[i].startsWith('-')) pos.push(args[i]);
  }
  if (pos[0] === 'pr' && pos[1] === 'merge') return 'gh pr merge is not allowed';
  if (pos[0] === 'api' && args.some((a) => /(^|\/)merges?(\?|$)|\/git\/refs(\/|\?|$)/.test(a))) {
    return 'gh api merge / git refs calls are not allowed';
  }
  return null;
}

// A word that may move the shell or point git elsewhere (`cd`, `GIT_DIR=`, `HOME=`, `env -C` …).
const MOVES = (w, next) => ['cd', 'pushd', 'popd'].includes(base(w)) || /^[A-Za-z_]\w*=/.test(w)
  || /^--chdir(=|$)/.test(w) || (base(w) === 'env' && (next ?? '').startsWith('-'));

// Reason the whole command text is refused, or null. The argument of `sh -c` / `eval` and
// command substitutions are scanned again; deeper nesting than that is refused.
function reason(text, cwd, depth = 0, movedOuter = false) {
  if (depth > 4) return 'command nesting is too deep to check';
  const { cmds, subs, unclosed } = commands(text);
  if (unclosed) return `heredoc <<${unclosed} has no closing line`;
  const moved = movedOuter || [...cmds, ...subs.map((s) => s.split(/\s+/))]
    .some((ws) => ws.some((w, j) => MOVES(w, ws[j + 1])));
  const ctx = { cwd, moved };
  for (const sub of subs) {
    const r = reason(sub, cwd, depth + 1, moved);
    if (r) return r;
  }
  for (const words of cmds) {
    for (let i = 0; i < words.length; i++) {
      const name = base(words[i]);
      let r = null;
      if (SHELLS.has(name)) {
        const k = words.findIndex((a, j) => j > i && /^-[a-z]*c[a-z]*$/.test(a));
        if (k > 0 && k + 1 < words.length) r = reason(words[k + 1], cwd, depth + 1, moved);
      } else if (name === 'eval') {
        r = reason(words.slice(i + 1).join(' '), cwd, depth + 1, moved);
      } else if (name === 'git') {
        r = gitReason(words.slice(i + 1), ctx);
      } else if (name === 'gh') {
        r = ghReason(words.slice(i + 1));
      }
      if (r) return r;
    }
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
