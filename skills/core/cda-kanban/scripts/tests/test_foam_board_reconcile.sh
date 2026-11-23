#!/usr/bin/env bash
# foam-board.sh reconcile: merged PR → card done; --pr never commits on the default
# branch, lands through orch_merge_pr (rebase merge), closes issues only after the merge.
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

TMP="$(cd "$(mktemp -d)" && pwd -P)"; trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin"
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export ORCH_PROGRESS=0 ORCH_TMUX=0
unset ORCH_REPO ORCH_DEFAULT_BRANCH CDAMUX_AGENT_ID

# Cards are updated 2026-11-10. Merged PRs:
#  #7  closes o/r#50, merged 11-12            → drift for card 50
#  #8  orch branch of foam-only, merged 11-12 → drift
#  #9  orch branch of 77 (card ready)         → never drift
#  #10 closes OTHER/repo#60                   → not evidence (other repo)
#  #11 orch branch of 61, merged 11-01        → older than the claim: not evidence
#  #12 orch branch of 62, base "release"      → not into the default branch
#  #13 / #14 orch branches of 63 / 64 (bad `updated`: the card's commit day, 11-05, rules)
cat >"$TMP/merged.json" <<'EOF'
[{"number":7,"headRefName":"worker/r1/x","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z",
  "closingIssuesReferences":[{"number":50,"repository":{"name":"r","owner":{"login":"O"}}}]},
 {"number":8,"headRefName":"feat/foam-only-issue-foam-only","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z","closingIssuesReferences":[]},
 {"number":9,"headRefName":"feat/77-issue-77","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z","closingIssuesReferences":[]},
 {"number":10,"headRefName":"worker/r1/y","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z",
  "closingIssuesReferences":[{"number":60,"repository":{"name":"repo","owner":{"login":"other"}}}]},
 {"number":11,"headRefName":"feat/61-issue-61","baseRefName":"main","mergedAt":"2026-11-01T10:00:00Z","closingIssuesReferences":[]},
 {"number":12,"headRefName":"feat/62-issue-62","baseRefName":"release","mergedAt":"2026-11-12T10:00:00Z","closingIssuesReferences":[]},
 {"number":13,"headRefName":"feat/63-issue-63","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z","closingIssuesReferences":[]},
 {"number":14,"headRefName":"feat/64-issue-64","baseRefName":"main","mergedAt":"2026-11-01T10:00:00Z","closingIssuesReferences":[]}]
EOF
cat >"$TMP/bin/gh" <<'STUB'
#!/usr/bin/env bash
echo "$*" >>"$GH_LOG"
board_branch() { git -C "$ORIGIN" for-each-ref --sort=-committerdate --count=1 --format='%(refname:short)' refs/heads/board/; }
case "$1 $2" in
  "repo view") case "$*" in *rebaseMergeAllowed*) echo true ;; *) echo "o/r main" ;; esac ;;
  "pr list")
    case "$*" in
      *"--state merged"*) [ "${GH_FAIL_LIST:-0}" = 1 ] && exit 1; cat "$MERGED_JSON" ;;
      *"--state open"*)
        q=""; while [ $# -gt 0 ]; do [ "$1" = -q ] && q="$2"; shift; done
        [ -f "$OPEN_PR" ] && jq -r "$q" "$OPEN_PR"; exit 0 ;;
    esac ;;
  "pr create") echo "https://github.com/o/r/pull/99" ;;
  "pr checks") echo "[]" ;;
  "pr view") echo '{"mergeable":"MERGEABLE","state":"OPEN"}' ;;
  "pr diff")
    case "$*" in *--name-only*) git -C "$ORIGIN" diff --name-only main "$(board_branch)" ;;
      *) git -C "$ORIGIN" diff main "$(board_branch)" ;; esac ;;
  "pr merge")
    case "$*" in *--rebase*--match-head-commit*) ;; *) echo "stub: merge must be --rebase --match-head-commit" >&2; exit 1 ;; esac
    [ "${GH_MERGE_FAIL:-0}" = 1 ] && { echo "stub: merge refused" >&2; exit 1; }
    touch "$MERGED_MARK" ;;
  "pr close")
    [ -f "$OPEN_PR" ] && git -C "$ORIGIN" branch -D "$(jq -r '.[0].headRefName' "$OPEN_PR")" >/dev/null
    rm -f "$OPEN_PR" ;;
  "issue view") echo OPEN ;;
  "issue close") [ -f "$MERGED_MARK" ] || echo "CLOSED-BEFORE-MERGE $*" >>"$GH_LOG" ;;
  *) echo "stub gh: unexpected $*" >&2; exit 2 ;;
esac
STUB
chmod +x "$TMP/bin/gh"
export PATH="$TMP/bin:$PATH" GH_LOG="$TMP/gh.log" MERGED_JSON="$TMP/merged.json" ORIGIN="$TMP/origin.git"
export OPEN_PR="$TMP/open-pr" MERGED_MARK="$TMP/merged"

card() { # file status [github_issue]
  {
    printf -- '---\ntitle: "t"\nstatus: %s\npriority: P1\nupdated: 2026-11-10\n' "$2"
    [ -z "${3:-}" ] || printf 'github_issue: %s\n' "$3"
    printf -- '---\n\nBody line status: in-review stays.\n'
  } >"$1"
}

git init -q -b main "$TMP/seed"
mkdir -p "$TMP/seed/foam/board"
card "$TMP/seed/foam/board/50-closed-by-pr.md" in-review 50
card "$TMP/seed/foam/board/foam-only.md" in-progress
card "$TMP/seed/foam/board/60-other-repo.md" in-review 60
card "$TMP/seed/foam/board/61-old-pr.md" in-progress 61
card "$TMP/seed/foam/board/62-release.md" in-review 62
card "$TMP/seed/foam/board/77-ready.md" ready 77
card "$TMP/seed/foam/board/63-bad-date.md" in-review 63
card "$TMP/seed/foam/board/64-bad-date-old-pr.md" in-review 64
sed -i.bak 's/^updated: .*/updated: soon/' "$TMP/seed/foam/board/63-bad-date.md" "$TMP/seed/foam/board/64-bad-date-old-pr.md"
rm -f "$TMP/seed/foam/board/"*.bak
printf 'orch: allow-no-checks\n' >"$TMP/seed/AGENTS.md"
printf '.orch/\n' >"$TMP/seed/.gitignore"
git -C "$TMP/seed" add -A
GIT_COMMITTER_DATE="2026-11-05T12:00:00Z" git -C "$TMP/seed" commit -qm init
git clone -q --bare "$TMP/seed" "$ORIGIN"
git clone -q "$ORIGIN" "$TMP/repo"
REPO="$TMP/repo"
B="$REPO/foam/board"
st() { awk '/^status:/ { print $2; exit }' "$1"; }

echo "# report"
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile)"
assert_contains "$out" "drift 50 in-review -> done (PR #7 merged)" "closing PR in the same repo is evidence"
assert_contains "$out" "drift foam-only in-progress -> done (PR #8 merged)" "orch branch feat/<id>-issue-<id> is evidence"
assert_absent "$out" "drift 60" "closing ref to another repo is not evidence"
assert_absent "$out" "drift 61" "PR merged before the card's updated day is not evidence"
assert_absent "$out" "drift 62" "PR into a non-default base is not evidence"
assert_absent "$out" "drift 77" "ready card is never drift"
assert_contains "$out" "drift 63 in-review -> done (PR #13 merged)" "bad updated: PR newer than the card's commit day is evidence"
assert_absent "$out" "drift 64" "bad updated: PR older than the card's commit day is not"
assert_contains "$(cat "$GH_LOG")" "pr list --state merged --base main" "merged list filtered on the default branch"
assert_eq "$(git -C "$REPO" status --porcelain)" "" "report: working tree untouched"

echo "# gh failure fails closed"
if (cd "$REPO" && GH_FAIL_LIST=1 bash "$FB" reconcile --write) 2>/dev/null; then
  printf 'FAIL reconcile must fail when gh cannot list merged PRs\n' >&2; FAILS=$((FAILS + 1))
else
  assert_eq "$(git -C "$REPO" status --porcelain)" "" "gh failure: board unchanged"
fi

echo "# --write"
(cd "$REPO" && bash "$FB" reconcile --write >/dev/null)
assert_eq "$(st "$B/50-closed-by-pr.md")/$(st "$B/foam-only.md")" "done/done" "write: drifted cards → done"
assert_eq "$(st "$B/60-other-repo.md")/$(st "$B/61-old-pr.md")/$(st "$B/62-release.md")" "in-review/in-progress/in-review" "write: others unchanged"
assert_contains "$(cat "$B/50-closed-by-pr.md")" "Body line status: in-review stays." "write: body kept"

echo "# status-only guard reads the index (untracked files included)"
# shellcheck source=../foam-board.sh
source "$FB"
git -C "$REPO" add -A foam/board
if reconcile_diff_is_status_only "$REPO" HEAD; then printf 'ok   guard accepts status/updated-only index\n'
else printf 'FAIL guard refused a status-only diff\n' >&2; FAILS=$((FAILS + 1)); fi
card "$B/new-card.md" done
git -C "$REPO" add -A foam/board
if reconcile_diff_is_status_only "$REPO" HEAD; then printf 'FAIL guard accepted a new untracked card\n' >&2; FAILS=$((FAILS + 1))
else printf 'ok   guard refuses a new (untracked) card\n'; fi
git -C "$REPO" reset -q --hard && git -C "$REPO" clean -qfd foam

echo "# reconcile lock: live pid and fresh pid-less dir hold, old pid-less dir expires"
L="$TMP/lock"
mkdir "$L"
if reconcile_lock "$L"; then printf 'FAIL fresh pid-less lock was taken\n' >&2; FAILS=$((FAILS + 1)); else printf 'ok   fresh pid-less lock holds\n'; fi
touch -t 202001010000 "$L"
if reconcile_lock "$L"; then printf 'ok   old pid-less lock expires\n'; else printf 'FAIL old pid-less lock never expires\n' >&2; FAILS=$((FAILS + 1)); fi
if reconcile_lock "$L"; then printf 'FAIL lock held by a live pid was taken\n' >&2; FAILS=$((FAILS + 1)); else printf 'ok   live pid holds\n'; fi
rm -rf "$L"

echo "# --pr: branch off origin/main, rebase merge, issues closed after the merge"
main_before="$(git -C "$ORIGIN" rev-parse main)"
local_before="$(git -C "$REPO" rev-parse HEAD)"
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile --pr 2>"$TMP/pr.err")"
pushed="$(git -C "$ORIGIN" for-each-ref --format='%(refname:short)' refs/heads/board/)"
assert_contains "$pushed" "board/reconcile-" "pr: reconcile branch pushed"
assert_eq "$(git -C "$ORIGIN" rev-parse main)" "$main_before" "pr: origin main never committed"
assert_eq "$(git -C "$REPO" rev-parse HEAD)" "$local_before" "pr: local checkout never committed"
assert_eq "$(git -C "$REPO" status --porcelain)" "" "pr: local working tree clean"
assert_eq "$(git -C "$REPO" worktree list | wc -l | tr -d ' ')" "1" "pr: throwaway worktree removed"
assert_eq "$(git -C "$REPO" for-each-ref refs/heads/board/)" "" "pr: local reconcile branch removed"
assert_eq "$(git -C "$ORIGIN" diff --name-only main "$pushed")" \
  "$(printf 'foam/board/50-closed-by-pr.md\nfoam/board/63-bad-date.md\nfoam/board/foam-only.md')" "pr: commit touches drifted cards only"
log="$(cat "$GH_LOG")"
assert_contains "$log" "pr create --base main --head board/reconcile-" "pr: PR opened against the default branch"
assert_contains "$log" "pr merge 99 --rebase --match-head-commit" "pr: landed with a rebase merge pinned to the head"
assert_contains "$log" "issue close 50" "pr: github_issue closed"
assert_absent "$log" "CLOSED-BEFORE-MERGE" "pr: issue closed only after the board PR merged"
assert_absent "$log" "issue close 60" "pr: card still in review keeps its issue open"

echo "# --pr: merge refused → PR left open, no issue closed"
git -C "$ORIGIN" branch -D "$pushed" >/dev/null
rm -f "$MERGED_MARK"
: >"$GH_LOG"
out="$(cd "$REPO" && GH_MERGE_FAIL=1 bash "$FB" reconcile --pr 2>&1)"
assert_contains "$out" "not merged" "merge failure reported"
assert_absent "$(cat "$GH_LOG")" "issue close" "no issue closed when the board PR did not merge"
pending="$(git -C "$ORIGIN" for-each-ref --format='%(refname:short)' refs/heads/board/)"

echo "# --pr with that PR still open: lands it, opens no duplicate"
echo "# a fork PR on a board/reconcile-* branch is never adopted"
printf '[{"number":%s,"headRefName":"%s","isCrossRepository":true}]\n' 41 "$pending" >"$OPEN_PR"
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile --pr 2>&1)"
assert_absent "$(cat "$GH_LOG")" "pr merge 41" "cross-repository PR ignored"
git -C "$ORIGIN" for-each-ref --format='%(refname:short)' refs/heads/board/ | grep -vx "$pending" | while read -r b; do git -C "$ORIGIN" branch -D "$b" >/dev/null; done
rm -f "$MERGED_MARK"

echo "# an open reconcile PR with extra commits is left for the maintainer"
git clone -q "$ORIGIN" "$TMP/meddle"
git -C "$TMP/meddle" checkout -q "$pending"
printf 'x\n' >>"$TMP/meddle/foam/board/60-other-repo.md"
git -C "$TMP/meddle" commit -qam meddle && git -C "$TMP/meddle" push -q origin "$pending"
printf '[{"number":%s,"headRefName":"%s","isCrossRepository":false}]\n' 41 "$pending" >"$OPEN_PR"
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile --pr 2>&1)"
assert_contains "$out" "holds more than status/updated lines" "tampered reconcile PR refused"
assert_absent "$(cat "$GH_LOG")" "pr merge" "tampered PR not merged"
assert_absent "$(cat "$GH_LOG")" "pr create" "no duplicate while it is open"
git -C "$TMP/meddle" reset -q --hard HEAD~1 && git -C "$TMP/meddle" push -q -f origin "$pending"

printf '[{"number":%s,"headRefName":"%s","isCrossRepository":false}]\n' 41 "$pending" >"$OPEN_PR"
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile --pr 2>&1)"
log="$(cat "$GH_LOG")"
assert_contains "$log" "pr merge 41 --rebase" "open reconcile PR is landed by the next reconcile"
assert_absent "$log" "pr create" "no duplicate reconcile PR"
assert_contains "$log" "issue close 50" "issues close once that PR merged"

echo "# --pr with an open PR behind main: closed and regenerated"
rm -f "$MERGED_MARK"
git -C "$TMP/seed" commit -q --allow-empty -m "main moves"
git -C "$TMP/seed" push -q "$ORIGIN" main
printf '[{"number":%s,"headRefName":"%s","isCrossRepository":false}]\n' 41 "$pending" >"$OPEN_PR"
: >"$GH_LOG"
out="$(cd "$REPO" && bash "$FB" reconcile --pr 2>&1)"
log="$(cat "$GH_LOG")"
assert_contains "$out" "closed stale reconcile PR #41" "behind PR closed"
assert_contains "$log" "pr close 41 --delete-branch" "stale branch deleted"
assert_contains "$log" "pr create --base main" "fresh reconcile PR opened"

if [ "$FAILS" -ne 0 ]; then printf '%s failed\n' "$FAILS" >&2; cat "$TMP/pr.err" >&2; exit 1; fi
printf 'all ok\n'
