#!/usr/bin/env bash
# foam-board.sh reconcile: merged PR → card done; --pr never commits on the default branch.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FB="$ROOT/foam-board.sh"

FAILS=0
assert_eq() {
  if [ "$1" = "$2" ]; then printf 'ok   %s\n' "$3"
  else printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$3" "$1" "$2" >&2; FAILS=$((FAILS + 1)); fi
}
assert_contains() {
  case "$1" in *"$2"*) printf 'ok   %s\n' "$3" ;;
  *) printf 'FAIL %s\n  missing: %s\n  in: %s\n' "$3" "$2" "$1" >&2; FAILS=$((FAILS + 1)) ;; esac
}
assert_absent() {
  case "$1" in *"$2"*) printf 'FAIL %s\n  unexpected: %s\n' "$3" "$2" >&2; FAILS=$((FAILS + 1)) ;;
  *) printf 'ok   %s\n' "$3" ;; esac
}

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin"
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

# Merged PRs: #7 closes issue 50; #8 is the orch.sh branch of the foam-only card.
cat >"$TMP/merged.json" <<'EOF'
[{"number":7,"headRefName":"worker/r1/x","closingIssuesReferences":[{"number":50}]},
 {"number":8,"headRefName":"feat/foam-only-issue-foam-only","closingIssuesReferences":[]},
 {"number":9,"headRefName":"feat/77-issue-77","closingIssuesReferences":[]}]
EOF
cat >"$TMP/bin/gh" <<'STUB'
#!/usr/bin/env bash
echo "$*" >>"$GH_LOG"
case "$1 $2" in
  "pr list")
    case "$*" in
      *"--state merged"*) [ "${GH_FAIL_LIST:-0}" = 1 ] && exit 1; cat "$MERGED_JSON" ;;
      *"--state open"*) printf '%s\n' "${GH_OPEN_RECONCILE:-}" ;;
    esac ;;
  "pr create") echo "https://github.com/o/r/pull/99" ;;
  "pr checks") echo "[]" ;;
  "pr diff")
    br="$(git -C "$ORIGIN" for-each-ref --format='%(refname:short)' refs/heads/board/ | head -n 1)"
    case "$*" in *--name-only*) git -C "$ORIGIN" diff --name-only main "$br" ;; *) git -C "$ORIGIN" diff main "$br" ;; esac ;;
  "pr merge") exit 0 ;;
  "issue view") echo OPEN ;;
  "issue close") exit 0 ;;
  "repo view") echo main ;;
  *) echo "stub gh: unexpected $*" >&2; exit 2 ;;
esac
STUB
chmod +x "$TMP/bin/gh"
export PATH="$TMP/bin:$PATH" GH_LOG="$TMP/gh.log" MERGED_JSON="$TMP/merged.json" ORIGIN="$TMP/origin.git"

card() { # file status [github_issue]
  {
    printf -- '---\ntitle: "t"\nstatus: %s\npriority: P1\nupdated: 2026-11-01\n' "$2"
    [ -z "${3:-}" ] || printf 'github_issue: %s\n' "$3"
    printf -- '---\n\nBody line status: in-review stays.\n'
  } >"$1"
}

# Origin (bare) + a clone: the board lives on origin/main.
git init -q -b main "$TMP/seed"
mkdir -p "$TMP/seed/foam/board"
card "$TMP/seed/foam/board/50-closed-by-pr.md" in-review 50
card "$TMP/seed/foam/board/foam-only.md" in-progress
card "$TMP/seed/foam/board/60-open.md" in-review 60
card "$TMP/seed/foam/board/77-ready.md" ready 77
printf 'orch: allow-no-checks\n' >"$TMP/seed/AGENTS.md"
git -C "$TMP/seed" add -A && git -C "$TMP/seed" commit -qm init
git clone -q --bare "$TMP/seed" "$TMP/origin.git"
git clone -q "$TMP/origin.git" "$TMP/repo"
REPO="$TMP/repo"
B="$REPO/foam/board"

# Report: lists drift, writes nothing.
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile)"
assert_contains "$out" "drift 50 in-review -> done (PR #7 merged)" "report: card closed by a merged PR"
assert_contains "$out" "drift foam-only in-progress -> done (PR #8 merged)" "report: orch.sh branch feat/<id>-issue-<id>"
assert_absent "$out" "60" "report: card without a merged PR is not drift"
assert_absent "$out" "77" "report: ready card is never drift"
assert_eq "$(git -C "$REPO" status --porcelain)" "" "report: working tree untouched"

# gh failure: fail closed.
if (cd "$REPO" && GH_FAIL_LIST=1 bash "$FB" reconcile --write) 2>"$TMP/err"; then
  printf 'FAIL reconcile must fail when gh cannot list merged PRs\n' >&2; FAILS=$((FAILS + 1))
else
  assert_eq "$(git -C "$REPO" status --porcelain)" "" "gh failure: board unchanged"
fi

# --write: only drifted cards move, body untouched.
(cd "$REPO" && bash "$FB" reconcile --write >/dev/null)
st() { awk '/^status:/ { print $2; exit }' "$1"; }
assert_eq "$(st "$B/50-closed-by-pr.md")" "done" "write: 50 → done"
assert_eq "$(st "$B/foam-only.md")" "done" "write: foam-only → done"
assert_eq "$(st "$B/60-open.md")" "in-review" "write: 60 unchanged"
assert_eq "$(st "$B/77-ready.md")" "ready" "write: 77 unchanged"
assert_contains "$(cat "$B/50-closed-by-pr.md")" "Body line status: in-review stays." "write: body kept"

# Status-only guard: a body edit is refused.
# shellcheck source=../foam-board.sh
source "$FB"
if reconcile_diff_is_status_only "$REPO" HEAD; then
  printf 'ok   guard accepts status/updated-only diff\n'
else
  printf 'FAIL guard refused a status-only diff\n' >&2; FAILS=$((FAILS + 1))
fi
printf 'extra\n' >>"$B/60-open.md"
if reconcile_diff_is_status_only "$REPO" HEAD; then
  printf 'FAIL guard accepted a body edit\n' >&2; FAILS=$((FAILS + 1))
else
  printf 'ok   guard refuses a body edit\n'
fi
git -C "$REPO" checkout -q -- .

# --pr with a reconcile PR already open: skipped, nothing pushed.
: >"$GH_LOG"
out="$(cd "$REPO" && GH_OPEN_RECONCILE=41 bash "$FB" reconcile --pr)"
assert_contains "$out" "reconcile PR #41 already open; skipped" "pr: open reconcile PR → skip"
assert_eq "$(git -C "$TMP/origin.git" branch --list 'board/*')" "" "pr: skip pushes nothing"

# --pr: branch off origin/main, PR, issue close, ADR-016 self-merge; default branch untouched.
main_before="$(git -C "$TMP/origin.git" rev-parse main)"
local_before="$(git -C "$REPO" rev-parse HEAD)"
: >"$GH_LOG"
out="$(cd "$REPO" && FOAM_BOARD_CHECKS_TIMEOUT=5 bash "$FB" reconcile --pr 2>"$TMP/pr.err")"
pushed="$(git -C "$TMP/origin.git" for-each-ref --format='%(refname:short)' 'refs/heads/board/')"
assert_contains "$pushed" "board/reconcile-" "pr: reconcile branch pushed"
assert_eq "$(git -C "$TMP/origin.git" rev-parse main)" "$main_before" "pr: origin main never committed"
assert_eq "$(git -C "$REPO" rev-parse HEAD)" "$local_before" "pr: local checkout never committed"
assert_eq "$(git -C "$REPO" status --porcelain)" "" "pr: local working tree clean"
assert_eq "$(git -C "$REPO" worktree list | wc -l | tr -d ' ')" "1" "pr: throwaway worktree removed"
if [ -n "$pushed" ]; then
  files="$(git -C "$TMP/origin.git" diff --name-only main "$pushed")"
  assert_eq "$files" "$(printf 'foam/board/50-closed-by-pr.md\nfoam/board/foam-only.md')" "pr: commit touches drifted cards only"
fi
log="$(cat "$GH_LOG")"
assert_contains "$log" "pr create --base main --head board/reconcile-" "pr: PR opened against the default branch"
assert_contains "$log" "issue close 50" "pr: github_issue closed"
assert_absent "$log" "issue close 60" "pr: issue of a card still in review stays open"
assert_contains "$log" "pr merge 99 --match-head-commit" "pr: ADR-016 self-merge pinned to the pushed head"
assert_contains "$out" "drift 50 in-review -> done (PR #7 merged)" "pr: drift reported"

if [ "$FAILS" -ne 0 ]; then printf '%s failed\n' "$FAILS" >&2; cat "$TMP/pr.err" >&2; exit 1; fi
printf 'all ok\n'
