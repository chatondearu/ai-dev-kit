// Where the map lives: project foam/ → $CDA_FOAM_HOME/projects/<repo>/ → <repo>/floorplan/.
import fs from 'node:fs';
import path from 'node:path';

export const MAP_DIR = 'floorplan';
export const LEGACY_MAP_DIR = 'show-code';

/** True for a folder holding a map (not a project folder that happens to be named floorplan). */
export const isMapDir = (d) =>
  ['architecture.json', 'plan.json', 'graph.json', '.cache'].some((f) => fs.existsSync(path.join(d, f)));

function repoName(root) {
  return path.basename(path.resolve(root));
}

/**
 * Resolve the map output directory.
 * Explicit `--out` wins. Else: `<root>/foam/floorplan` when foam/ exists;
 * else `$CDA_FOAM_HOME/projects/<repo>/floorplan` when CDA_FOAM_HOME is a directory;
 * else `<root>/floorplan` (upstream default, with show-code/ migrate).
 */
export function resolveOutDir(root, { out, migrate = true, env = process.env } = {}) {
  if (out) return path.resolve(out);

  const foam = path.join(root, 'foam');
  if (fs.existsSync(foam) && fs.statSync(foam).isDirectory()) {
    return path.join(foam, MAP_DIR);
  }

  const home = env.CDA_FOAM_HOME;
  if (home && fs.existsSync(home) && fs.statSync(home).isDirectory()) {
    return path.join(home, 'projects', repoName(root), MAP_DIR);
  }

  const dir = path.join(root, MAP_DIR);
  const old = path.join(root, LEGACY_MAP_DIR);
  if (migrate && !fs.existsSync(dir) && isMapDir(old)) {
    fs.renameSync(old, dir);
    console.error(`(moved the existing map from ${LEGACY_MAP_DIR}/ to ${MAP_DIR}/)`);
  }
  if (fs.existsSync(dir) && !isMapDir(dir) && fs.readdirSync(dir).length) {
    console.error(
      `✗ ${MAP_DIR}/ already exists here and isn't a map (it's part of the project). Pass --out <folder> to put the map somewhere else.`,
    );
    process.exit(1);
  }
  return dir;
}

/** Which resolution path was chosen (for notes / gitignore policy). */
export function outDirKind(root, outDir, env = process.env) {
  const abs = path.resolve(outDir);
  const foamMap = path.resolve(root, 'foam', MAP_DIR);
  if (abs === foamMap) return 'foam';
  const home = env.CDA_FOAM_HOME;
  if (home) {
    const homeMap = path.resolve(home, 'projects', repoName(root), MAP_DIR);
    if (abs === homeMap) return 'cda-foam-home';
  }
  return 'project';
}

/** Path of the foam index note next to the map folder (foam/floorplan.md or …/floorplan.md). */
export function foamNotePath(root, outDir, env = process.env) {
  const kind = outDirKind(root, outDir, env);
  if (kind === 'foam') return path.join(root, 'foam', 'floorplan.md');
  if (kind === 'cda-foam-home') {
    return path.join(env.CDA_FOAM_HOME, 'projects', repoName(root), 'floorplan.md');
  }
  return null;
}

const FOAM_GITIGNORE = `# cda-floorplan generated files — keep architecture.json, descriptions/, plan.json
index.html
artifact.html
graph.json
query.mjs
serve.mjs
.cache/
custom-flows.json
recent-work.json
`;

/** Selective ignore under foam / CDA_FOAM_HOME; full self-ignore under project floorplan/. */
export function writeMapGitignore(root, outDir, env = process.env) {
  const kind = outDirKind(root, outDir, env);
  fs.mkdirSync(outDir, { recursive: true });
  const gi = path.join(outDir, '.gitignore');
  if (kind === 'foam' || kind === 'cda-foam-home') {
    const want = FOAM_GITIGNORE;
    try {
      if (!fs.existsSync(gi) || fs.readFileSync(gi, 'utf8') !== want) fs.writeFileSync(gi, want);
    } catch {}
    return null;
  }
  // Upstream: ignore the whole map folder (including this .gitignore).
  const want = '# Floorplan keeps its map out of git: this ignores everything in this folder, this file included.\n*\n';
  try {
    if (!fs.existsSync(gi) || fs.readFileSync(gi, 'utf8') !== want) fs.writeFileSync(gi, want);
  } catch {}
  return null;
}

/** Wikilinks-friendly foam note pointing at the map. No-op when map is plain <repo>/floorplan/. */
export function writeFoamNote(root, outDir, env = process.env) {
  const note = foamNotePath(root, outDir, env);
  if (!note) return 'skipped (no foam / CDA_FOAM_HOME)';
  const kind = outDirKind(root, outDir, env);
  const mapRel =
    kind === 'foam'
      ? 'floorplan/'
      : path.relative(path.dirname(note), outDir).replace(/\\/g, '/') + '/';
  const body = `# Floorplan

Interactive architecture map for this project.

- Map folder: \`${mapRel}\`
- Architecture: [[${mapRel}architecture.json]]
- Descriptions: \`${mapRel}descriptions/\`

After a build, query with \`node ${mapRel}query.mjs overview\` and open with \`node ${mapRel}serve.mjs\`.

Generated HTML, graph.json, and helpers stay gitignored; commit architecture.json, descriptions/, and this note.
`;
  fs.mkdirSync(path.dirname(note), { recursive: true });
  const prev = fs.existsSync(note) ? fs.readFileSync(note, 'utf8') : null;
  if (prev === body) return `${path.basename(note)} unchanged`;
  fs.writeFileSync(note, body);
  return `${prev == null ? 'created' : 'updated'} ${path.relative(root, note).replace(/\\/g, '/') || note}`;
}
