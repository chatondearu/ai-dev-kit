/**
 * Pure session-list helpers for the tailnet dashboard (and node selfcheck).
 * Nesting matches Fleet `session_forest`: dead parents / cycle members → roots.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  } else {
    root.cdamuxSessionTree = api;
  }
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  function sessionBadges(s) {
    const parts = [];
    if (s.unread > 0) parts.push(`✉${s.unread}`);
    if (s.pending_ask) parts.push("?");
    return parts;
  }

  function sessionGlyph(s) {
    return s.role === "orchestrator" ? "◎" : "•";
  }

  /**
   * Nest under `@cdamux_parent`. Fixture a↔b + c→a → roots [a,b], c under a
   * (Rust: session_links::cycle_members_become_roots_and_keep_their_children).
   */
  function nestSessions(sessions) {
    const list = sessions || [];
    const index = new Map(list.map((s, i) => [s.name, i]));
    const parentOf = (i) => {
      const name = list[i].parent;
      if (!name || !index.has(name)) return null;
      const p = index.get(name);
      return p === i ? null : p;
    };
    const children = list.map(() => []);
    const roots = [];
    for (let i = 0; i < list.length; i++) {
      let cur = parentOf(i);
      let onCycle = false;
      for (let n = 0; n < list.length; n++) {
        if (cur === null) break;
        if (cur === i) {
          onCycle = true;
          break;
        }
        cur = parentOf(cur);
      }
      const p = parentOf(i);
      if (p !== null && !onCycle) children[p].push(i);
      else roots.push(i);
    }
    const out = [];
    const walk = (i, depth) => {
      out.push({ session: list[i], depth });
      for (const kid of children[i]) walk(kid, depth + 1);
    };
    for (const r of roots) walk(r, 0);
    return out;
  }

  return { nestSessions, sessionBadges, sessionGlyph };
});
