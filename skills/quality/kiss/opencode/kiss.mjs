// kiss — OpenCode plugin. Injects the KISS ruleset into every chat's system
// prompt at the active level and toggles level on /kiss. Reuses the shared
// builder in ../hooks/lib.js so every host reads one source of truth (SKILL.md).
// Adapted from ponytail (https://github.com/DietrichGebert/ponytail, MIT).
//
// Enable in opencode.json:  { "plugins": ["./kiss.mjs"] }  (or drop it in
// ~/.config/opencode/plugin/). install.sh links it there for you.

import { createRequire } from 'node:module';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Resolve the REAL path first: opencode loads this via a symlink from its plugin
// dir, and `..` must resolve against the repo, not the symlink location.
const realPath = fs.realpathSync(fileURLToPath(import.meta.url));
const require = createRequire(realPath);
const { getInstructions, normalizeMode, DEFAULT_MODE } =
  require(path.join(path.dirname(realPath), '..', 'hooks', 'lib.js'));

// OpenCode keeps its own mode flag beside its config (the node hooks' flag lives
// under the Claude/Cursor dirs, not here).
const statePath = path.join(
  process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'),
  'opencode', '.kiss-active',
);

function readMode() {
  try { return normalizeMode(fs.readFileSync(statePath, 'utf8')) || DEFAULT_MODE; }
  catch (e) { return DEFAULT_MODE; }
}
function writeMode(mode) {
  fs.mkdirSync(path.dirname(statePath), { recursive: true });
  fs.writeFileSync(statePath, mode);
}
// Bare /kiss turns it on (or keeps the level); /kiss <level|off> sets it.
function persistMode(args) {
  const wanted = String(args == null ? '' : args).trim().toLowerCase();
  if (!wanted && readMode() !== 'off') return;
  const mode = wanted ? normalizeMode(wanted) : DEFAULT_MODE;
  if (mode) writeMode(mode);
}

function addRuleset(event) {
  const mode = readMode();
  if (mode === 'off') return;
  event.system.push({ type: 'text', text: getInstructions(mode) });
}

export default {
  id: 'kiss',

  async setup(ctx) {
    await ctx.command.transform((editor) => {
      editor.add({
        name: 'kiss',
        description: 'Toggle the KISS code-attitude level (lite|full|ultra|off).',
        execute: async ({ sessionID, prompt, delivery }) => {
          persistMode(prompt.text);
          await ctx.session.prompt({ ...prompt, sessionID, text: prompt.text, delivery });
        },
      });
    });
    await ctx.session.hook('context', addRuleset);
    await ctx.session.hook('compaction', addRuleset);
  },

  // OpenCode V1 fallback.
  async server() {
    return {
      'experimental.chat.system.transform': async (_input, output) => {
        const mode = readMode();
        if (mode === 'off') return;
        const instructions = getInstructions(mode);
        if (output.system.length > 0) output.system[output.system.length - 1] += '\n\n' + instructions;
        else output.system.push(instructions);
      },
      'command.execute.before': async (input) => {
        if (input && input.command === 'kiss') persistMode(input.arguments);
      },
    };
  },
};
