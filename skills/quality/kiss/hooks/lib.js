// kiss — shared hook library (config + mode state + instruction builder + repo map).
// Adapted from ponytail (https://github.com/DietrichGebert/ponytail, MIT),
// trimmed to this kit: Claude Code + Cursor host the node hooks; opencode uses
// its own plugin (../opencode/kiss.mjs). One source of truth = ../SKILL.md.

const fs = require('fs');
const path = require('path');
const os = require('os');
const { createHash } = require('crypto');
const { execFileSync } = require('child_process');

const DEFAULT_MODE = 'full';
const MODES = new Set(['lite', 'full', 'ultra', 'off']);
const SKILL_PATH = path.join(__dirname, '..', 'SKILL.md');
const STATE_FILE = '.kiss-active';

function normalizeMode(s) {
  const m = String(s || '').trim().toLowerCase();
  return MODES.has(m) ? m : null;
}

// Host detection: Cursor sets CURSOR_VERSION only in its hook env; otherwise Claude.
const isCursor = Boolean(process.env.CURSOR_VERSION);

function configDir() {
  if (isCursor) return path.join(os.homedir(), '.cursor');
  return process.env.CLAUDE_CONFIG_DIR || path.join(os.homedir(), '.claude');
}

// Per-project flag (Claude passes CLAUDE_PROJECT_DIR) so repos don't clobber each
// other; plus a shared flag the statusline / project-less hosts read.
function statePaths() {
  const dir = configDir();
  const shared = path.join(dir, STATE_FILE);
  const proj = (process.env.CLAUDE_PROJECT_DIR || '').trim();
  const perProject = proj
    ? path.join(dir, 'kiss-modes', createHash('sha256').update(path.normalize(proj)).digest('hex'))
    : null;
  return { shared, perProject };
}

function readMode() {
  const { shared, perProject } = statePaths();
  try {
    return normalizeMode(fs.readFileSync(perProject || shared, 'utf8')) || null;
  } catch (e) {
    return null;
  }
}

function setMode(mode) {
  const { shared, perProject } = statePaths();
  for (const file of [perProject, shared]) {
    if (!file) continue;
    try {
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, mode);
    } catch (e) { /* best-effort */ }
  }
}

function clearMode() {
  const { shared, perProject } = statePaths();
  for (const file of [perProject, shared]) {
    if (file) try { fs.unlinkSync(file); } catch (e) { /* already gone */ }
  }
}

// Keep only the active level's row in the Levels table; drop the other modes'
// rows. Ordinary rule bullets (not keyed by a mode word) survive verbatim.
function filterByMode(body, mode) {
  const effective = normalizeMode(mode) || DEFAULT_MODE;
  return String(body || '')
    .replace(/^---[\s\S]*?---\s*/, '')
    .split(/\r?\n/)
    .filter((line) => {
      const row = line.match(/^\|\s*\*\*(.+?)\*\*\s*\|/);
      if (row) {
        const label = normalizeMode(row[1].trim());
        if (label) return label === effective;
      }
      return true;
    })
    .join('\n');
}

function getInstructions(mode) {
  const effective = normalizeMode(mode) || DEFAULT_MODE;
  try {
    return 'KISS MODE ACTIVE — level: ' + effective + '\n\n' +
      filterByMode(fs.readFileSync(SKILL_PATH, 'utf8'), effective);
  } catch (e) {
    return 'KISS MODE ACTIVE — level: ' + effective +
      '. Solve the task with the least new code; reuse before writing; never cut ' +
      'validation, error handling, security or accessibility. End with what you skipped + any risk.';
  }
}

// Cheap repo map so "reuse first" costs no search. git-tracked paths, char-budgeted.
function buildMap(cwd = process.cwd()) {
  const budget = Number(process.env.KISS_MAP_CHARS || 2000);
  try {
    const files = execFileSync('git', ['ls-files'], {
      cwd, encoding: 'utf8', timeout: 3000, maxBuffer: 4 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'ignore'],
    }).split('\n').filter(Boolean);
    if (!files.length) return '';
    let out = 'Repo map (reuse what already exists before writing):\n';
    for (const f of files) {
      if (out.length + f.length + 1 > budget) { out += '…(truncated)'; break; }
      out += f + '\n';
    }
    return out.trimEnd();
  } catch (e) {
    return '';
  }
}

// Synchronous: hooks call process.exit right after (a pending async pipe write
// can die with the process).
function writeHookOutput(event, context) {
  if (!context) { if (isCursor) fs.writeSync(1, '{}'); return; }
  if (isCursor) {
    const out = { additional_context: context };
    if (event === 'UserPromptSubmit' || event === 'beforeSubmitPrompt') out.continue = true;
    fs.writeSync(1, JSON.stringify(out));
    return;
  }
  // Native Claude: SessionStart accepts raw stdout; SubagentStart needs JSON.
  if (event === 'SubagentStart') {
    fs.writeSync(1, JSON.stringify({ hookSpecificOutput: { hookEventName: event, additionalContext: context } }));
    return;
  }
  fs.writeSync(1, context);
}

// Parse a /kiss command or a deactivation phrase from the user prompt.
// Returns { mode } to switch, or null for no-op.
function parseCommand(prompt) {
  const p = String(prompt || '').trim().toLowerCase();
  if (/^(stop kiss|normal mode|kiss off)\b/.test(p)) return { mode: 'off' };
  const m = p.match(/^\/kiss(?:\s+(lite|full|ultra|off))?\b/);
  if (m) return { mode: normalizeMode(m[1]) || DEFAULT_MODE };
  return null;
}

module.exports = {
  DEFAULT_MODE, isCursor, normalizeMode,
  readMode, setMode, clearMode,
  getInstructions, buildMap, writeHookOutput, parseCommand,
};
