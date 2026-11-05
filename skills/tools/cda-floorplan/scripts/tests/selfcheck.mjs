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
const { detectAgent, AGENTS, DETECT_ORDER, ASK_ENGINES } = await import(pathToFileURL(path.join(LIB, 'agent.mjs')).href);

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
// Ask bar: structured adapters only — never treat agent/cursor-agent/opencode as claudeRun
assert.deepEqual(ASK_ENGINES, ['claude', 'codex']);
for (const bad of ['agent', 'cursor-agent', 'opencode']) {
  assert.equal(ASK_ENGINES.includes(bad), false);
}

assert.equal(detectAgent({ engine: 'opencode' }, {}), 'opencode');
assert.equal(detectAgent({ agent: 'cursor-agent' }, {}), 'cursor-agent');
assert.equal(detectAgent({}, { FLOORPLAN_ENGINE: 'codex' }), 'codex');
assert.equal(detectAgent({}, { FLOORPLAN_AGENT: 'agent' }), 'agent');
assert.equal(detectAgent({}, { CODEX_THREAD_ID: 't1' }), 'codex');
// empty PATH → default claude
assert.equal(detectAgent({}, { PATH: '' }), 'claude');

// --- serve root + query path under foam (map parent ≠ codebase root) ---
{
  // Contract mirrored by assets/serve.mjs load(): root = meta.root, mapRel = meta.mapDir || relative(root, mapDir)
  const resolveServe = (mapDir) => {
    const g = JSON.parse(fs.readFileSync(path.join(mapDir, 'graph.json'), 'utf8'));
    const root = path.resolve(g.meta.root);
    const mapRel = (g.meta.mapDir || path.relative(root, mapDir).replace(/\\/g, '/') || 'floorplan').replace(/\/$/, '');
    return { root, mapRel };
  };

  const root = mkTmp('serve-foam');
  const mapDir = path.join(root, 'foam', 'floorplan');
  fs.mkdirSync(mapDir, { recursive: true });
  fs.writeFileSync(
    path.join(mapDir, 'graph.json'),
    JSON.stringify({ meta: { root, mapDir: 'foam/floorplan' } }),
  );
  const { root: gotRoot, mapRel } = resolveServe(mapDir);
  assert.equal(gotRoot, root);
  assert.notEqual(gotRoot, path.dirname(mapDir)); // must not be foam/
  assert.equal(mapRel, 'foam/floorplan');
  assert.equal(path.join(gotRoot, mapRel, 'query.mjs'), path.join(mapDir, 'query.mjs'));

  // CDA_FOAM_HOME: map outside the repo — relative path escapes; still ≠ dirname(mapDir) as cwd
  const home = mkTmp('serve-home');
  const homeMap = path.join(home, 'projects', path.basename(root), 'floorplan');
  fs.mkdirSync(homeMap, { recursive: true });
  const homeMapRel = path.relative(root, homeMap).replace(/\\/g, '/');
  fs.writeFileSync(
    path.join(homeMap, 'graph.json'),
    JSON.stringify({ meta: { root, mapDir: homeMapRel } }),
  );
  const homeServe = resolveServe(homeMap);
  assert.equal(homeServe.root, root);
  assert.notEqual(homeServe.root, path.dirname(homeMap));
  assert.equal(homeServe.mapRel, homeMapRel);

  // serve.mjs / graph-query must not hardcode floorplan/query for allowlist / ask instructions
  const assets = path.join(HERE, '..', '..', 'assets');
  const serveSrc = fs.readFileSync(path.join(assets, 'serve.mjs'), 'utf8');
  assert.match(serveSrc, /G\.meta\?\.root/);
  assert.match(serveSrc, /mapRel/);
  assert.doesNotMatch(serveSrc, /Bash\(node floorplan\/query\.mjs/);
  // Ask must not spawn agent/cursor-agent/opencode through claudeRun flags
  assert.match(serveSrc, /ASK_ENGINES\s*=\s*\[\s*['"]claude['"]\s*,\s*['"]codex['"]\s*\]/);
  assert.match(serveSrc, /for \(const k of askOrder\)/);
  assert.match(serveSrc, /engine === 'codex' \? codexRun : claudeRun/);
  assert.doesNotMatch(serveSrc, /share -p style/);
  assert.doesNotMatch(serveSrc, /ENGINES\.agent\s*=/);
  assert.doesNotMatch(serveSrc, /ENGINES\['cursor-agent'\]|ENGINES\["cursor-agent"\]/);
  const gq = fs.readFileSync(path.join(assets, 'graph-query.js'), 'utf8');
  assert.match(gq, /mapDirRel|meta\?\.mapDir/);
  assert.doesNotMatch(gq, /node floorplan\/query\.mjs overview/);
}

console.log('cda-floorplan self-check: ok');
