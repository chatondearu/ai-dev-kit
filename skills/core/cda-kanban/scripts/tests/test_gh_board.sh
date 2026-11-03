#!/usr/bin/env bash
# Tests for gh-board.sh: reconcile git signal, events (reviews + cursor), pagination.
# Uses a stub `gh` on PATH and temp git repos; never touches the network.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=../gh-board.sh
source "$ROOT/gh-board.sh"

FAILS=0
assert_eq() {
  if [ "$1" = "$2" ]; then printf 'ok   %s\n' "$3"
  else printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$3" "$1" "$2" >&2; FAILS=$((FAILS + 1)); fi
}

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
mkdir "$TMP/bin"

# Stub gh: behaviour driven by files in $STUB (timeline.json, page1.json, page2.json, fail).
cat > "$TMP/bin/gh" <<'STUB'
#!/usr/bin/env bash
case "$1 $2" in
  "project view") echo "PID";;
  "project field-list") echo '{"fields":[{"name":"Status","id":"F1","options":[{"id":"o1","name":"Ready"},{"id":"o2","name":"Done"}]}]}';;
  "project item-list") echo '{"items":[{"content":{"number":9,"title":"t","url":"https://github.com/o/r/pull/9"},"status":"Ready"}]}';;
  "api --paginate") [ -e "$STUB/fail" ] && exit 1; cat "$STUB/timeline.json";;
  "api graphql") if [[ "$*" == *"after=c1"* ]]; then cat "$STUB/page2.json"; else cat "$STUB/page1.json"; fi;;
  *) echo "stub gh: unexpected $*" >&2; exit 2;;
esac
STUB
chmod +x "$TMP/bin/gh"
export PATH="$TMP/bin:$PATH" STUB="$TMP"

# --- 1. reconcile git signal: only Closes/Fixes/Resolves count ---------------
git init -q "$TMP/repo"
(
  cd "$TMP/repo"
  git config user.email t@t; git config user.name t
  git commit -q --allow-empty -m "feat: a" -m "Closes #5"
  git commit -q --allow-empty -m "chore: mentions #6 in passing"
  git commit -q --allow-empty -m "docs: x" -m "Refs #7"
  git commit -q --allow-empty -m "fix: y" -m "fixes #50"
  BASE_REF=HEAD
  r() { if git_mentions_issue "$1"; then echo yes; else echo no; fi; }
  assert_eq "$(r 5)" yes "Closes #5 counts"
  assert_eq "$(r 6)" no "bare #6 does not count"
  assert_eq "$(r 7)" no "Refs #7 does not count"
  assert_eq "$(r 50)" yes "case-insensitive fixes #50"
  assert_eq "$(r 1)" no "#1 does not match #50 / #5"
)

# --- 2. events: review events kept; failed fetch keeps the cursor ------------
cat > "$TMP/timeline.json" <<'JSON'
[{"event":"reviewed","state":"approved","submitted_at":"2026-11-02T10:00:00Z","user":{"login":"bob"},"body":"lgtm","html_url":"u"}]
JSON
mkdir "$TMP/ev" && git init -q "$TMP/ev"
run_events() {
  (cd "$TMP/ev" && OVR_REPO=o/r OVR_PNUM=1 OVR_SINCE="" OVR_SINCEFILE="$TMP/cursor" OUT_JSON=1 LIMIT="" cmd_events 2>/dev/null)
}
echo "2026-11-01T00:00:00Z" > "$TMP/cursor"
out="$(run_events)"
assert_eq "$(printf '%s' "$out" | jq -r '.[0].type + " " + .[0].ts')" "review_approved 2026-11-02T10:00:00Z" "review event (submitted_at) kept"
[ "$(cat "$TMP/cursor")" != "2026-11-01T00:00:00Z" ] && c=advanced || c=stuck
assert_eq "$c" advanced "cursor advances on success"

echo "2026-11-01T00:00:00Z" > "$TMP/cursor"; touch "$TMP/fail"
run_events >/dev/null && rc=0 || rc=$?
rm "$TMP/fail"
assert_eq "$rc" 1 "events fails when a timeline fetch fails"
assert_eq "$(cat "$TMP/cursor")" "2026-11-01T00:00:00Z" "cursor kept after fetch failure"

# --- 3. pagination: list-ready and item-id walk every page, repo-filtered ----
item() { # number repo status
  printf '{"id":"I%s%s","fieldValueByName":{"name":"%s"},"content":{"__typename":"Issue","number":%s,"title":"t","state":"OPEN","repository":{"nameWithOwner":"%s"}}}' "$1" "${2//\//_}" "$3" "$1" "$2"
}
printf '{"data":{"node":{"items":{"pageInfo":{"hasNextPage":true,"endCursor":"c1"},"nodes":[%s,%s]}}}}' \
  "$(item 1 o/r Ready)" "$(item 5 other/x Ready)" > "$TMP/page1.json"
printf '{"data":{"node":{"items":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s,%s]}}}}' \
  "$(item 2 o/r Ready)" "$(item 5 o/r Backlog)" > "$TMP/page2.json"
ready="$(OVR_REPO=o/r OVR_PNUM=1 OUT_JSON=0 LIMIT="" cmd_list_ready 2>/dev/null | paste -sd, -)"
assert_eq "$ready" "1,2" "list-ready reads page 2"
id="$(OVR_REPO=o/r OVR_PNUM=1 LIMIT="" cmd_item_id 5 2>/dev/null)"
assert_eq "$id" "I5o_r" "item-id: one id, this repo only, found on page 2"

if [ "$FAILS" -ne 0 ]; then printf '%s failed\n' "$FAILS" >&2; exit 1; fi
printf 'all ok\n'
