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
assert.strictEqual(L.parseCommand("fix the login bug"), null);

// getInstructions: header + level-filtered table (keeps active row, drops others)
const lite = L.getInstructions("lite");
assert.ok(lite.startsWith("KISS MODE ACTIVE — level: lite"));
assert.ok(lite.includes("**lite**") && !lite.includes("**ultra**"), "lite keeps only its row");
assert.ok(L.getInstructions("ultra").includes("**ultra**"), "ultra keeps its row");

// mode persistence round-trip
L.setMode("ultra"); assert.strictEqual(L.readMode(), "ultra");
L.clearMode();      assert.strictEqual(L.readMode(), null);

console.log("kiss lib: all assertions passed");
'
