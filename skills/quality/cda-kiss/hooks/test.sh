#!/usr/bin/env bash
# kiss hooks — smoke test for the shared lib (pure logic, no host config touched).
set -euo pipefail
cd "$(dirname "$0")"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
export CLAUDE_CONFIG_DIR="$tmp"          # keep state writes out of the real config
unset CLAUDE_PROJECT_DIR CURSOR_VERSION ORCH_RUN_ID CDA_ROUTING 2>/dev/null || true

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
delete process.env.KISS_MAP_CHARS;

// Graphify SessionStart orientation: off / on+no-graph / on+graph
delete process.env.CDA_GRAPHIFY;
assert.strictEqual(L.buildGraphPointer(repo), "");
assert.ok(L.buildSessionMap(repo).startsWith("Repo map"), "off → repo map");

process.env.CDA_GRAPHIFY = "1";
assert.strictEqual(L.buildGraphPointer(repo), "", "on + no graph → empty");
assert.ok(L.buildSessionMap(repo).startsWith("Repo map"), "on + no graph → repo map");

fs.mkdirSync(path.join(repo, "graphify-out"), { recursive: true });
fs.writeFileSync(path.join(repo, "graphify-out", "graph.json"), "{}");
const ptr = L.buildGraphPointer(repo);
assert.ok(ptr.includes("GRAPH_REPORT.md") && ptr.includes("graph.json"), ptr);
assert.strictEqual(L.buildSessionMap(repo), ptr, "on + graph replaces repo map");
delete process.env.CDA_GRAPHIFY;

console.log("kiss lib: all assertions passed");
'

# sticky off round trip: off -> fresh SessionStart is silent -> /kiss re-enables
export CLAUDE_PROJECT_DIR="$tmp/proj"
echo '{"prompt":"stop kiss"}' | node mode-tracker.js >/dev/null
out="$(echo '{"source":"startup"}' | KISS_MAP=0 node activate.js)"
echo "$out" | grep -q "KISS MODE ACTIVE" && { echo "FAIL: activate.js kept the ruleset after off"; exit 1; }
echo "$out" | grep -q "CDA ROUTING" || { echo "FAIL: routing dropped with kiss off"; exit 1; }
echo '{"prompt":"/kiss"}' | node mode-tracker.js >/dev/null
echo '{"source":"startup"}' | KISS_MAP=0 node activate.js | grep -q "KISS MODE ACTIVE — level: full" \
  || { echo "FAIL: /kiss did not re-enable"; exit 1; }
echo "kiss hooks: sticky off round trip passed"

# Routing: on by default, silent inside orch.sh runs and with CDA_ROUTING=0,
# carried in Cursor's additional_context
out="$(echo '{"source":"startup"}' | KISS_MAP=0 node activate.js)"
echo "$out" | grep -q "CDA ROUTING" || { echo "FAIL: routing missing at session start"; exit 1; }
echo "$out" | grep -q '`cda-dev`' || { echo "FAIL: routing does not name cda-dev"; exit 1; }
for v in ORCH_RUN_ID=run1 CDA_ROUTING=0; do
  out="$(echo '{"source":"startup"}' | env "$v" KISS_MAP=0 node activate.js)"
  echo "$out" | grep -q "CDA ROUTING" && { echo "FAIL: routing injected with $v"; exit 1; }
  echo "$out" | grep -q "KISS MODE ACTIVE" || { echo "FAIL: ruleset lost with $v"; exit 1; }
done
echo '{"source":"startup"}' | HOME="$tmp" CURSOR_VERSION=1 KISS_MAP=0 node activate.js \
  | node -e 'let s="";process.stdin.on("data",c=>s+=c).on("end",()=>{if(!JSON.parse(s).additional_context.includes("CDA ROUTING"))process.exit(1)})' \
  || { echo "FAIL: cursor additional_context lacks routing"; exit 1; }
echo "kiss hooks: routing cases passed"

# Graphify activate.js: on + graph injects pointer (cwd = fixture with graph.json)
hooks_dir="$PWD"
grepo="$tmp/graph-repo"
mkdir -p "$grepo/graphify-out" "$grepo/src"
printf '%s\n' '{}' > "$grepo/graphify-out/graph.json"
printf '%s\n' 'x' > "$grepo/src/a.js"
git -C "$grepo" init -q
git -C "$grepo" add .
out="$(cd "$grepo" && echo '{"source":"startup"}' | CDA_GRAPHIFY=1 node "$hooks_dir/activate.js")"
echo "$out" | grep -q "GRAPH_REPORT.md" || { echo "FAIL: activate on+graph missing pointer"; exit 1; }
echo "$out" | grep -q "Repo map" && { echo "FAIL: activate on+graph still has repo map"; exit 1; }
out="$(cd "$grepo" && echo '{"source":"startup"}' | env -u CDA_GRAPHIFY node "$hooks_dir/activate.js")"
echo "$out" | grep -q "Repo map" || { echo "FAIL: activate off missing repo map"; exit 1; }
echo "$out" | grep -q "GRAPH_REPORT.md" && { echo "FAIL: activate off has graph pointer"; exit 1; }
rm -f "$grepo/graphify-out/graph.json"
out="$(cd "$grepo" && echo '{"source":"startup"}' | CDA_GRAPHIFY=1 node "$hooks_dir/activate.js")"
echo "$out" | grep -q "Repo map" || { echo "FAIL: activate on+no-graph missing repo map"; exit 1; }
echo "$out" | grep -q "GRAPH_REPORT.md" && { echo "FAIL: activate on+no-graph has pointer"; exit 1; }
echo "kiss hooks: graphify SessionStart cases passed"
