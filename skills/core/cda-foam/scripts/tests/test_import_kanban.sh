#!/usr/bin/env bash
# import-kanban.sh lists only COMPLETED closed issues as shipped (stub gh, no network).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin" "$TMP/repo/foam"
git -C "$TMP/repo" init -q
cat >"$TMP/bin/gh" <<'STUB'
#!/usr/bin/env bash
case "$1 $2" in
  "repo view") echo o/r ;;
  "issue list") echo '[{"number":1,"title":"done","state":"CLOSED","stateReason":"COMPLETED"},
    {"number":2,"title":"nope","state":"CLOSED","stateReason":"NOT_PLANNED"},
    {"number":3,"title":"dupe","state":"CLOSED","stateReason":"DUPLICATE"}]' ;;
esac
STUB
chmod +x "$TMP/bin/gh"
PATH="$TMP/bin:$PATH" bash "$ROOT/skills/core/cda-foam/scripts/import-kanban.sh" "$TMP/repo" >/dev/null
S="$TMP/repo/foam/features/shipped.md"
grep -q '#1\]' "$S" || { echo "FAIL: completed issue missing" >&2; exit 1; }
! grep -qE '#[23]\]' "$S" || { echo "FAIL: not-planned/duplicate listed as shipped" >&2; exit 1; }
echo "summary: all ok"
