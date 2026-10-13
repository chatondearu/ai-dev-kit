#!/usr/bin/env bash
# kiss hooks — smoke test for the shared lib (pure logic, no host config touched).
set -euo pipefail
cd "$(dirname "$0")"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
export CLAUDE_CONFIG_DIR="$tmp"          # keep state writes out of the real config
unset CLAUDE_PROJECT_DIR CURSOR_VERSION 2>/dev/null || true

node -e '
const assert = require("assert");
const L = require("./lib.js");

// normalizeMode
assert.strictEqual(L.normalizeMode("ULTRA"), "ultra");
assert.strictEqual(L.normalizeMode("full"), "full");
assert.strictEqual(L.normalizeMode("nope"), null);

// parseCommand
assert.deepStrictEqual(L.parseCommand("/kiss ultra"), { mode: "ultra" });
assert.deepStrictEqual(L.parseCommand("/kiss"), { mode: "full" });
assert.deepStrictEqual(L.parseCommand("stop kiss"), { mode: "off" });
assert.deepStrictEqual(L.parseCommand("normal mode"), { mode: "off" });
assert.deepStrictEqual(L.parseCommand("  /KISS   ULTRA "), { mode: "ultra" });
assert.deepStrictEqual(L.parseCommand("/kiss off"), { mode: "off" });
assert.deepStrictEqual(L.parseCommand("kiss off"), { mode: "off" });
assert.strictEqual(L.parseCommand("fix the login bug"), null);
assert.strictEqual(L.parseCommand(""), null);
for (const p of ["/kiss-audit", "/kiss-audit src/", "/kiss-x", "/kiss ultra please"])
  assert.strictEqual(L.parseCommand(p), null, p);

// getInstructions: header + level-filtered table (keeps active row, drops others)
const lite = L.getInstructions("lite");
assert.ok(lite.startsWith("KISS MODE ACTIVE — level: lite"));
assert.ok(lite.includes("**lite**") && !lite.includes("**ultra**"), "lite keeps only its row");
assert.ok(L.getInstructions("ultra").includes("**ultra**"), "ultra keeps its row");

// mode persistence round-trip
L.setMode("ultra"); assert.strictEqual(L.readMode(), "ultra");

// dir-level repo map: unique dirs (depth <= 2), not files, within budget
const { execFileSync } = require("child_process");
const fs = require("fs"), path = require("path");
const repo = fs.mkdtempSync(path.join(process.env.CLAUDE_CONFIG_DIR, "repo-"));
for (const f of ["README.md", "a/b/c/x.js", "a/b/y.js", "a/z.js", "d/w.js"]) {
  fs.mkdirSync(path.dirname(path.join(repo, f)), { recursive: true });
  fs.writeFileSync(path.join(repo, f), "");
}
execFileSync("git", ["init", "-q"], { cwd: repo });
execFileSync("git", ["add", "."], { cwd: repo });
const map = L.buildMap(repo);
assert.deepStrictEqual(map.split("\n").slice(1), ["a/b", "a", "d"], map);
process.env.KISS_MAP_CHARS = "60";
const small = L.buildMap(repo);
assert.ok(small.endsWith("…(truncated)") && small.length <= 60 + 12, small);

console.log("kiss lib: all assertions passed");
'

# sticky off round trip: off -> fresh SessionStart is silent -> /kiss re-enables
export CLAUDE_PROJECT_DIR="$tmp/proj"
echo '{"prompt":"stop kiss"}' | node mode-tracker.js >/dev/null
out="$(echo '{"source":"startup"}' | KISS_MAP=0 node activate.js)"
[ -z "$out" ] || { echo "FAIL: activate.js not silent after off"; exit 1; }
echo '{"prompt":"/kiss"}' | node mode-tracker.js >/dev/null
echo '{"source":"startup"}' | KISS_MAP=0 node activate.js | grep -q "KISS MODE ACTIVE — level: full" \
  || { echo "FAIL: /kiss did not re-enable"; exit 1; }
echo "kiss hooks: sticky off round trip passed"
