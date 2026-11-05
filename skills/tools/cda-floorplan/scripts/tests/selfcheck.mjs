#!/usr/bin/env node
// Assert-based self-check for output resolution and engine detection (PLAN-187).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const LIB = path.join(HERE, '..', 'lib');

const { resolveOutDir, outDirKind, foamNotePath, writeFoamNote, writeMapGitignore } = await import(
  pathToFileURL(path.join(LIB, 'outdir.mjs')).href
);
const { detectAgent, AGENTS, DETECT_ORDER } = await import(pathToFileURL(path.join(LIB, 'agent.mjs')).href);

function mkTmp(name) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `cda-floorplan-${name}-`));
}

// --- output resolution: foam → CDA_FOAM_HOME → fallback ---
{
  const root = mkTmp('foam');
  fs.mkdirSync(path.join(root, 'foam'));
  const out = resolveOutDir(root, { env: {} });
  assert.equal(out, path.join(root, 'foam', 'floorplan'));
  assert.equal(outDirKind(root, out, {}), 'foam');
  assert.equal(foamNotePath(root, out, {}), path.join(root, 'foam', 'floorplan.md'));
  writeMapGitignore(root, out, {});
  assert.match(fs.readFileSync(path.join(out, '.gitignore'), 'utf8'), /architecture\.json/);
  const noteMsg = writeFoamNote(root, out, {});
  assert.match(noteMsg, /created|updated/);
  assert.ok(fs.existsSync(path.join(root, 'foam', 'floorplan.md')));
}

{
  const root = mkTmp('home');
  const home = mkTmp('cdahome');
  const env = { CDA_FOAM_HOME: home };
  const out = resolveOutDir(root, { env });
  assert.equal(out, path.join(home, 'projects', path.basename(root), 'floorplan'));
  assert.equal(outDirKind(root, out, env), 'cda-foam-home');
  writeFoamNote(root, out, env);
  assert.ok(fs.existsSync(path.join(home, 'projects', path.basename(root), 'floorplan.md')));
}

{
  const root = mkTmp('plain');
  const out = resolveOutDir(root, { env: {} });
  assert.equal(out, path.join(root, 'floorplan'));
  assert.equal(outDirKind(root, out, {}), 'project');
  assert.equal(foamNotePath(root, out, {}), null);
  assert.equal(writeFoamNote(root, out, {}), 'skipped (no foam / CDA_FOAM_HOME)');
  writeMapGitignore(root, out, {});
  assert.match(fs.readFileSync(path.join(out, '.gitignore'), 'utf8'), /(^|\n)\*\n?$/);
}

{
  const root = mkTmp('explicit');
  const custom = path.join(root, 'custom-map');
  assert.equal(resolveOutDir(root, { out: custom, env: { CDA_FOAM_HOME: '/tmp' } }), path.resolve(custom));
}

{
  // foam wins over CDA_FOAM_HOME
  const root = mkTmp('foamwins');
  fs.mkdirSync(path.join(root, 'foam'));
  const home = mkTmp('ignored-home');
  const out = resolveOutDir(root, { env: { CDA_FOAM_HOME: home } });
  assert.equal(out, path.join(root, 'foam', 'floorplan'));
}

// --- engine detection order + --engine override ---
assert.deepEqual(DETECT_ORDER, ['agent', 'cursor-agent', 'claude', 'opencode', 'codex']);
assert.deepEqual(AGENTS, DETECT_ORDER);

assert.equal(detectAgent({ engine: 'opencode' }, {}), 'opencode');
assert.equal(detectAgent({ agent: 'cursor-agent' }, {}), 'cursor-agent');
assert.equal(detectAgent({}, { FLOORPLAN_ENGINE: 'codex' }), 'codex');
assert.equal(detectAgent({}, { FLOORPLAN_AGENT: 'agent' }), 'agent');
assert.equal(detectAgent({}, { CODEX_THREAD_ID: 't1' }), 'codex');
// empty PATH → default claude
assert.equal(detectAgent({}, { PATH: '' }), 'claude');

console.log('cda-floorplan self-check: ok');
