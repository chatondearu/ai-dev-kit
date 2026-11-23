#!/usr/bin/env node
// Worker push guard (ADR-019): Claude Code PreToolUse hook on Bash, wired by the rendered
// worker --settings file as `node worker-push-guard.mjs <own-branch> <default-branch>`.
// Bash permission rules match command text; this parses argv so spellings the rules miss
// (`-uf`, `:ref`, quoted refs, `git -C x push`, `bash -c "…"`) are still refused.
// Blocks (exit 2, reason on stderr): any `git push` / `send-pack` that is not an explicit
// push of the own branch (no force, delete, mirror/all/tags, no default branch),
// `git -c alias.*`, `gh pr merge` and `gh api` merge / git-refs paths. Conservative: an
// unquoted `git` word anywhere in a command is treated as a call.
// Text is not a sandbox: a script the worker writes can still push; branch protection is
// the real guard.
import { readFileSync } from 'node:fs';

const [own = '', dflt = ''] = process.argv.slice(2);

// Drop heredoc bodies with a quoted delimiter (`<<'EOF'`): their text is data, never run.
function stripHeredocs(text) {
  const lines = text.split('\n');
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    out.push(lines[i]);
    const m = lines[i].match(/<<-?\s*(['"])(\w+)\1/);
    if (!m) continue;
    while (i + 1 < lines.length && lines[i + 1].trim() !== m[2]) i++;
  }
  return out.join('\n');
}

// Text of the `$(…)` starting at text[i] ('$'), and the index of its closing paren.
function substitution(text, i) {
  let depth = 0;
  for (let j = i + 1; j < text.length; j++) {
    if (text[j] === '(') depth++;
    else if (text[j] === ')' && --depth === 0) return [text.slice(i + 2, j), j];
  }
  return [text.slice(i + 2), text.length];
}

// Split shell text into simple commands of words. Quote-aware; operators, parens, braces
// and unquoted `$(` / backticks separate commands. `$(…)` and backticks inside double
// quotes run too: their text goes to `subs` to be scanned as commands. Other quoted text
// (commit messages, PR bodies, grep patterns) is data.
function commands(text) {
  const cmds = [];
  const subs = [];
  let words = [];
  let w = null;
  const flushWord = () => { if (w !== null) words.push(w); w = null; };
  const flushCmd = () => { flushWord(); if (words.length) cmds.push(words); words = []; };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === "'") {
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
  return { cmds, subs };
}

const base = (word) => word.split('/').pop();
const OPT_WITH_VALUE = new Set(['-C', '-c', '--git-dir', '--work-tree', '--namespace', '--config-env']);
const PUSH_OK_LONG = new Set(['--set-upstream', '--dry-run', '--quiet', '--verbose', '--progress',
  '--no-progress', '--no-verify', '--atomic', '--porcelain']);

// Reason a `git` call (words after `git`) is refused, or null.
function gitReason(args) {
  let i = 0;
  for (; i < args.length && args[i].startsWith('-'); i++) {
    const [opt, val] = args[i].split(/=(.*)/s);
    const value = val ?? (OPT_WITH_VALUE.has(opt) ? args[++i] ?? '' : '');
    if (opt === '-c' && /^alias\./i.test(value)) return 'git -c alias.* is not allowed';
    if (opt === '--config-env') return 'git --config-env is not allowed';
  }
  if (args[i] !== 'push' && args[i] !== 'send-pack') return null;
  const positional = [];
  for (const a of args.slice(i + 1)) {
    if (a === '-o' || a.startsWith('--push-option')) return 'push options are not allowed';
    if (a.startsWith('--')) {
      if (!PUSH_OK_LONG.has(a.split('=')[0])) return `git push ${a} is not allowed`;
    } else if (a.startsWith('-') && a.length > 1) {
      if (/[^unqv]/.test(a.slice(1))) return `git push ${a} is not allowed`;
    } else {
      positional.push(a);
    }
  }
  if (!own || own === dflt) return 'this worker may not push (no own branch)';
  if (positional.length < 2) return `name the branch: git push origin ${own}`;
  for (const spec of positional.slice(1)) {
    if (spec.startsWith('+')) return `force refspec ${spec} is not allowed`;
    const k = spec.indexOf(':');
    let dst = k < 0 ? spec : spec.slice(k + 1);
    if (k === 0 || dst === '') return `deleting a ref (${spec}) is not allowed`;
    dst = dst.replace(/^refs\/heads\//, '');
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

// Reason the whole command text is refused, or null. The argument of `sh -c` / `eval` and
// command substitutions are scanned again; deeper nesting than that is refused.
function reason(text, depth = 0) {
  if (depth > 4) return 'command nesting is too deep to check';
  const { cmds, subs } = commands(stripHeredocs(text));
  for (const sub of subs) {
    const r = reason(sub, depth + 1);
    if (r) return r;
  }
  for (const words of cmds) {
    for (let i = 0; i < words.length; i++) {
      const name = base(words[i]);
      let r = null;
      if (SHELLS.has(name)) {
        const k = words.findIndex((a, j) => j > i && /^-[a-z]*c[a-z]*$/.test(a));
        if (k > 0 && k + 1 < words.length) r = reason(words[k + 1], depth + 1);
      } else if (name === 'eval') {
        r = reason(words.slice(i + 1).join(' '), depth + 1);
      } else if (name === 'git') {
        r = gitReason(words.slice(i + 1));
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
const r = cmd && reason(cmd);
if (r) {
  process.stderr.write(`worker push guard: ${r} (ADR-019). A denied command is an escalation `
    + 'to the maintainer through the orchestrator; never work around it.\n');
  process.exit(2);
}
