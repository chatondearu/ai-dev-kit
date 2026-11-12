#!/usr/bin/env node
/** Selfcheck for session_tree.js (nesting cycles + badges). */
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const { nestSessions, sessionBadges, sessionGlyph } = require(
  join(dirname(fileURLToPath(import.meta.url)), "session_tree.js")
);

function assert(cond, msg) {
  if (!cond) throw new Error(msg);
}

const nested = nestSessions([
  { name: "orch", parent: null, role: "orchestrator", unread: 2, pending_ask: false },
  { name: "w1", parent: "orch", role: "worker", unread: 0, pending_ask: true },
]);
assert(nested.map((r) => `${r.depth}:${r.session.name}`).join(",") === "0:orch,1:w1", "nest");
assert(sessionGlyph(nested[0].session) === "◎", "orch glyph");
assert(sessionBadges(nested[0].session).join(" ") === "✉2", "unread badge");
assert(sessionBadges(nested[1].session).join(" ") === "?", "ask badge");

const cycle = nestSessions([
  { name: "a", parent: "b" },
  { name: "b", parent: "a" },
  { name: "c", parent: "a" },
]);
assert(
  cycle.map((r) => `${r.depth}:${r.session.name}`).join(",") === "0:a,1:c,0:b",
  `cycle got ${cycle.map((r) => `${r.depth}:${r.session.name}`).join(",")}`
);
assert(cycle.length === 3, "cycle keeps every session once");

console.log("session_tree_selfcheck: ok");
