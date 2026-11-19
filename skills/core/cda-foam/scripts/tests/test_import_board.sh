#!/usr/bin/env bash
# import-board.sh is a one-shot map: creates missing cards, never overwrites.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin" "$TMP/repo/foam/board"
git -C "$TMP/repo" init -q

cat >"$TMP/repo/foam/board/1-already.md" <<'EOF'
---
title: "keep me"
status: ready
priority: P0
updated: 2026-11-19
github_issue: 1
---
EOF

# ADR-016: {slug}.md with github_issue is a valid card (no N- prefix).
cat >"$TMP/repo/foam/board/unprefixed.md" <<'EOF'
---
title: "keep unprefixed"
status: backlog
priority: P2
updated: 2026-11-19
github_issue: 2
---
EOF

cat >"$TMP/bin/gh" <<'STUB'
#!/usr/bin/env bash
case "$1 $2" in
  "issue list") echo '[{"number":1,"title":"already"},{"number":2,"title":"feat(x): New Card"},{"number":3,"title":"feat(y): Third"}]' ;;
  *) echo "stub gh: $*" >&2; exit 2 ;;
esac
STUB
chmod +x "$TMP/bin/gh"

PATH="$TMP/bin:$PATH" bash "$ROOT/skills/core/cda-foam/scripts/import-board.sh" "$TMP/repo" >/dev/null
grep -q 'keep me' "$TMP/repo/foam/board/1-already.md" || { echo "FAIL: existing card overwritten" >&2; exit 1; }
grep -q 'keep unprefixed' "$TMP/repo/foam/board/unprefixed.md" || { echo "FAIL: unprefixed card overwritten" >&2; exit 1; }
[ ! -e "$TMP/repo/foam/board/2-feat-x-new-card.md" ] || { echo "FAIL: created 2-*.md despite github_issue: 2 on unprefixed.md" >&2; exit 1; }
[ -f "$TMP/repo/foam/board/3-feat-y-third.md" ] || { echo "FAIL: new card for #3 missing" >&2; exit 1; }
grep -q 'github_issue: 3' "$TMP/repo/foam/board/3-feat-y-third.md" || { echo "FAIL: github_issue missing" >&2; exit 1; }
grep -q 'status: backlog' "$TMP/repo/foam/board/3-feat-y-third.md" || { echo "FAIL: status not backlog" >&2; exit 1; }

# Second run is a no-op for existing files.
PATH="$TMP/bin:$PATH" bash "$ROOT/skills/core/cda-foam/scripts/import-board.sh" "$TMP/repo" >/dev/null
n="$(find "$TMP/repo/foam/board" -name '*.md' | wc -l | tr -d ' ')"
[ "$n" = "3" ] || { echo "FAIL: second run created extras (n=$n)" >&2; exit 1; }

echo "summary: all ok"
