#!/usr/bin/env bash
# foam-board.sh: list-ready from foam cards, claim/status, no Projects API.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIX="$(cd "$(dirname "$0")/fixtures/foam-board" && pwd)"
# shellcheck source=../foam-board.sh
source "$ROOT/foam-board.sh"

FAILS=0
assert_eq() {
  if [ "$1" = "$2" ]; then printf 'ok   %s\n' "$3"
  else printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$3" "$1" "$2" >&2; FAILS=$((FAILS + 1)); fi
}

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
cp -R "$FIX/." "$TMP/repo"
mkdir -p "$TMP/bin"

# gh stub: fail closed on Projects; record comments; optional issue state.
cat >"$TMP/bin/gh" <<'STUB'
#!/usr/bin/env bash
echo "$*" >>"${GH_LOG:-/tmp/gh-foam.log}"
case "$1 $2" in
  "project "*|"project") echo "Projects API must not be called" >&2; exit 99 ;;
  "issue comment") exit "${GH_COMMENT_RC:-0}" ;;
  "issue view")
    if [ "${GH_ISSUE_STATE:-}" = "CLOSED" ]; then echo CLOSED; exit 0; fi
    echo OPEN; exit 0
    ;;
  *) echo "stub gh: unexpected $*" >&2; exit 2 ;;
esac
STUB
chmod +x "$TMP/bin/gh"
export PATH="$TMP/bin:$PATH" GH_LOG="$TMP/gh.log"
: >"$GH_LOG"

export OVR_ROOT="$TMP/repo"

ready="$(OUT_JSON=0 cmd_list_ready | paste -sd, -)"
assert_eq "$ready" "10" "list-ready: ready card only (backlog + blocked skipped)"

# README / _template with status ready must not list.
assert_eq "$(fm_get "$TMP/repo/foam/board/_template.md" status)" "ready" "fixture trap: _template is ready"
case ",$ready," in
  *,_template,*|*README*) printf 'FAIL list-ready leaked README/_template\n' >&2; FAILS=$((FAILS + 1)) ;;
  *) printf 'ok   skips README and _template\n' ;;
esac

if command -v jq >/dev/null 2>&1; then
  js="$(OUT_JSON=1 cmd_list_ready)"
  assert_eq "$(printf '%s' "$js" | jq -r '.[0].id')" "10" "json id is github_issue"
  assert_eq "$(printf '%s' "$js" | jq -r 'length')" "1" "json one ready card"
fi

# Foam-only slug (no github_issue) is listed.
cat >"$TMP/repo/foam/board/foam-only.md" <<'EOF'
---
title: "feat: foam-only"
status: ready
priority: P2
updated: 2026-11-19
---
EOF
ready2="$(OUT_JSON=0 cmd_list_ready | sort | paste -sd, -)"
assert_eq "$ready2" "10,foam-only" "foam-only card listed as slug"

# Parser
assert_eq "$(fm_get "$TMP/repo/foam/board/ready-item.md" github_issue)" "10" "fm_get github_issue"
assert_eq "$(fm_get "$TMP/repo/foam/board/blocked-item.md" title)" "feat: blocked by backlog card" "fm_get quoted title stripped"

# set-status rewrites frontmatter, not Projects.
path="$(cmd_set_status 10 in-progress)"
assert_eq "$(fm_get "$path" status)" "in-progress" "set-status writes in-progress"
grep -q '^updated: ' "$path" || { echo "FAIL updated missing" >&2; FAILS=$((FAILS + 1)); }
if grep -q 'project' "$GH_LOG"; then
  printf 'FAIL set-status called gh project\n' >&2; FAILS=$((FAILS + 1))
else
  printf 'ok   set-status does not call Projects\n'
fi

# Restore ready for claim tests.
update_fm_field "$path" status ready

# Claim with github_issue comments; never creates an issue.
: >"$GH_LOG"
claimed="$(cmd_claim 10)"
assert_eq "$(fm_get "$claimed" status)" "in-progress" "claim sets in-progress"
if grep -q 'issue comment' "$GH_LOG" && ! grep -q 'issue create' "$GH_LOG" && ! grep -q project "$GH_LOG"; then
  printf 'ok   claim comments, no issue create, no Projects\n'
else
  printf 'FAIL claim gh log:\n%s\n' "$(cat "$GH_LOG")" >&2; FAILS=$((FAILS + 1))
fi

# Foam-only claim never calls gh.
: >"$GH_LOG"
cmd_claim foam-only >/dev/null
if [ -s "$GH_LOG" ]; then
  printf 'FAIL foam-only claim called gh:\n%s\n' "$(cat "$GH_LOG")" >&2; FAILS=$((FAILS + 1))
else
  printf 'ok   foam-only claim does not call gh\n'
fi
assert_eq "$(fm_get "$TMP/repo/foam/board/foam-only.md" status)" "in-progress" "foam-only claim writes card"

# blocked_by on a done card unblocks.
update_fm_field "$TMP/repo/foam/board/backlog-item.md" status done
update_fm_field "$TMP/repo/foam/board/blocked-item.md" status ready
unblocked="$(OUT_JSON=0 cmd_list_ready | sort | paste -sd, -)"
case ",$unblocked," in
  *,blocked-item,*) printf 'ok   blocked card listed when blocker is done\n' ;;
  *) printf 'FAIL expected blocked-item after done blocker, got %s\n' "$unblocked" >&2; FAILS=$((FAILS + 1)) ;;
esac

# Prefix glob must not steal an exact slug (import-board vs import-board-fix).
cat >"$TMP/repo/foam/board/import-board.md" <<'EOF'
---
title: "feat: import-board"
status: ready
priority: P1
updated: 2026-11-19
---
EOF
cat >"$TMP/repo/foam/board/import-board-fix.md" <<'EOF'
---
title: "fix: child of import-board"
status: ready
priority: P1
updated: 2026-11-19
blocked_by: [import-board]
---
EOF
got="$(find_card import-board)"
assert_eq "$got" "$TMP/repo/foam/board/import-board.md" "find_card exact slug, not import-board-fix.md"
child_blocked="$(OUT_JSON=0 cmd_list_ready | sort | paste -sd, -)"
case ",$child_blocked," in
  *,import-board-fix,*) printf 'FAIL import-board-fix listed while blocker ready, got %s\n' "$child_blocked" >&2; FAILS=$((FAILS + 1)) ;;
  *) printf 'ok   import-board-fix hidden while import-board is not done\n' ;;
esac
update_fm_field "$TMP/repo/foam/board/import-board.md" status done
child_ready="$(OUT_JSON=0 cmd_list_ready | sort | paste -sd, -)"
case ",$child_ready," in
  *,import-board-fix,*) printf 'ok   import-board-fix listed after import-board is done\n' ;;
  *) printf 'FAIL expected import-board-fix after done blocker, got %s\n' "$child_ready" >&2; FAILS=$((FAILS + 1)) ;;
esac

# Skill contract: orchestrator names the index path (ADR-015 / #407 file).
SKILL_ROOT="$(cd "$ROOT/../.." && pwd)"
if grep -q 'board-index.json' "$SKILL_ROOT/cda-orchestrator/SKILL.md"; then
  printf 'ok   cda-orchestrator names board-index.json\n'
else
  printf 'FAIL cda-orchestrator SKILL.md missing board-index.json\n' >&2; FAILS=$((FAILS + 1))
fi

if [ "$FAILS" -ne 0 ]; then printf '%s failed\n' "$FAILS" >&2; exit 1; fi
printf 'all ok\n'
