#!/usr/bin/env bash
# foam-board.sh — foam/board Ready oracle and status moves (ADR-016).
#
# Default list-ready does not call GitHub Projects. GitHub Issues are opt-in
# via card frontmatter `github_issue` (comments / Closes only). Never creates
# issues. Status moves rewrite the card file; the agent commits it.
#
# Usage:
#   ./foam-board.sh list-ready [--json] [--root DIR]
#   ./foam-board.sh set-status <id> <status> [--root DIR]
#   ./foam-board.sh claim <id> [--root DIR]
#   ./foam-board.sh path <id> [--root DIR]
#   ./foam-board.sh reconcile [--write | --pr] [--root DIR]
#
# reconcile: an in-progress|in-review card whose PR merged into the default
# branch on or after the card's `updated` day is drift. Default prints it;
# --write marks those cards done in the working tree; --pr does it on a branch
# off origin/<default>, opens a foam-only PR, tries once to rebase-merge it
# (ADR-016 §3b) and closes the cards' issues after that merge.
#
# Requires: bash. jq only for --json and reconcile. gh only for claim comments,
# reconcile, and blocked_by numbers that do not match a foam card.
set -euo pipefail

err() { printf 'foam-board: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"; }

VALID_STATUS='backlog|ready|in-progress|in-review|done'

repo_root() {
  if [ -n "${OVR_ROOT:-}" ]; then
    (cd "$OVR_ROOT" && pwd)
    return
  fi
  git rev-parse --show-toplevel 2>/dev/null || pwd
}

board_dir() {
  printf '%s/foam/board\n' "$(repo_root)"
}

# YAML frontmatter between the first two --- lines. Comments and missing keys
# yield empty. Strips one matching pair of quotes around the value.
fm_get() {
  local file="$1" key="$2"
  awk -v key="$key" '
    BEGIN { c=0 }
    /^---[[:space:]]*$/ { c++; if (c==2) exit; next }
    c==1 && $0 ~ /^[[:space:]]*#/ { next }
    c==1 && $0 ~ ("^" key ":") {
      sub("^" key ":[[:space:]]*", "")
      sub(/[[:space:]]+$/, "")
      if ((substr($0,1,1)=="\"" && substr($0,length($0),1)=="\"") ||
          (substr($0,1,1)=="'\''" && substr($0,length($0),1)=="'\''")) {
        $0 = substr($0, 2, length($0)-2)
      }
      print
      exit
    }
  ' "$file"
}

# blocked_by: [404, slug] or blocked_by: 404 → one ref per line.
fm_blocked_by() {
  local file="$1" raw
  raw="$(fm_get "$file" blocked_by)"
  [ -n "$raw" ] || return 0
  printf '%s\n' "$raw" | tr -d '[]' | tr ',' '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | grep -v '^$' || true
}

is_card_file() {
  local base
  base="$(basename "$1")"
  case "$base" in
    README.md|_*) return 1 ;;
  esac
  [ -f "$1" ] || return 1
  [ -n "$(fm_get "$1" status)" ] || return 1
  [ -n "$(fm_get "$1" priority)" ] || return 1
  [ -n "$(fm_get "$1" updated)" ] || return 1
  return 0
}

# Print card paths, one per line. Missing board dir → empty (not an error).
card_files() {
  local dir f
  dir="$(board_dir)"
  [ -d "$dir" ] || return 0
  shopt -s nullglob
  for f in "$dir"/*.md; do
    is_card_file "$f" || continue
    printf '%s\n' "$f"
  done
}

card_id() {
  local file="$1" gi
  gi="$(fm_get "$file" github_issue)"
  if [ -n "$gi" ]; then
    printf '%s\n' "$gi"
    return
  fi
  basename "$file" .md
}

# Resolve id to a card path: exact basename or github_issue first; numeric
# "{N}-*.md" prefix only if want is all digits and no exact match.
find_card() {
  local want="$1" f base gi
  [ -n "$want" ] || return 1
  if [ -f "$want" ]; then
    printf '%s\n' "$want"
    return 0
  fi
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    base="$(basename "$f" .md)"
    gi="$(fm_get "$f" github_issue)"
    if [ "$base" = "$want" ] || [ "$gi" = "$want" ]; then
      printf '%s\n' "$f"
      return 0
    fi
  done < <(card_files)
  case "$want" in
    ''|*[!0-9]*) return 1 ;;
  esac
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    base="$(basename "$f" .md)"
    case "$base" in
      "$want"-*) printf '%s\n' "$f"; return 0 ;;
    esac
  done < <(card_files)
  return 1
}

# True (exit 0) when ref still blocks: non-done card, or open/unknown GH issue.
blocker_open() {
  local ref="$1" card st state
  if card="$(find_card "$ref")"; then
    st="$(fm_get "$card" status)"
    [ "$st" != "done" ]
    return
  fi
  case "$ref" in
    ''|*[!0-9]*) return 0 ;;
  esac
  command -v gh >/dev/null 2>&1 || return 0
  state="$(gh issue view "$ref" --json state -q .state 2>/dev/null || true)"
  case "$state" in
    CLOSED) return 1 ;;
    *) return 0 ;;
  esac
}

card_is_blocked() {
  local file="$1" ref
  while IFS= read -r ref; do
    [ -n "$ref" ] || continue
    if blocker_open "$ref"; then
      return 0
    fi
  done < <(fm_blocked_by "$file")
  return 1
}

emit_ready_row() {
  local f="$1" root="$2" st id title gi prio file_rel
  st="$(fm_get "$f" status)"
  [ "$st" = "ready" ] || return 0
  card_is_blocked "$f" && return 0
  id="$(card_id "$f")"
  if [ "${OUT_JSON:-0}" = 1 ]; then
    title="$(fm_get "$f" title)"
    gi="$(fm_get "$f" github_issue)"
    prio="$(fm_get "$f" priority)"
    file_rel="${f#"$root"/}"
    jq -nc \
      --arg id "$id" --arg file "$file_rel" --arg title "$title" \
      --arg status "$st" --arg priority "$prio" --arg gi "$gi" \
      '{id:$id, file:$file, title:$title, status:$status, priority:$priority,
        github_issue: (if $gi == "" then null else $gi end)}'
  else
    printf '%s\n' "$id"
  fi
}

cmd_list_ready() {
  local f root
  root="$(repo_root)"
  if [ "${OUT_JSON:-0}" = 1 ]; then
    need jq
    {
      while IFS= read -r f; do
        [ -n "$f" ] || continue
        emit_ready_row "$f" "$root"
      done < <(card_files)
    } | jq -s '.'
  else
    while IFS= read -r f; do
      [ -n "$f" ] || continue
      emit_ready_row "$f" "$root"
    done < <(card_files)
  fi
}

update_fm_field() {
  local file="$1" key="$2" value="$3" tmp
  tmp="$(mktemp)"
  awk -v k="$key" -v v="$value" '
    BEGIN { c=0; found=0 }
    /^---[[:space:]]*$/ {
      c++
      if (c==2 && !found) print k ": " v
      print
      next
    }
    c==1 && $0 ~ ("^" k ":") { print k ": " v; found=1; next }
    { print }
  ' "$file" >"$tmp"
  mv "$tmp" "$file"
}

cmd_path() {
  local id="${1:-}" card
  [ -n "$id" ] || die "usage: foam-board.sh path <id>"
  card="$(find_card "$id")" || die "no foam card for '$id'"
  printf '%s\n' "$card"
}

cmd_set_status() {
  local id="${1:-}" status="${2:-}" card
  [ -n "$id" ] && [ -n "$status" ] || die "usage: foam-board.sh set-status <id> <status>"
  printf '%s' "$status" | grep -Eq "^($VALID_STATUS)$" \
    || die "invalid status '$status' (use $VALID_STATUS)"
  card="$(find_card "$id")" || die "no foam card for '$id'"
  update_fm_field "$card" status "$status"
  update_fm_field "$card" updated "$(date -u +%Y-%m-%d)"
  printf '%s\n' "$card"
}

cmd_claim() {
  local id="${1:-}" card st gi
  [ -n "$id" ] || die "usage: foam-board.sh claim <id>"
  card="$(find_card "$id")" || die "no foam card for '$id'"
  st="$(fm_get "$card" status)"
  case "$st" in
    ready|in-progress) ;;
    *) die "card $id is '$st', not ready" ;;
  esac
  gi="$(fm_get "$card" github_issue)"
  if [ -n "$gi" ]; then
    need gh
    gh issue comment "$gi" --body "Claimed for implementation." \
      || die "gh issue comment failed for #$gi"
  fi
  cmd_set_status "$id" in-progress >/dev/null
  printf '%s\n' "$card"
}

# Default branch and owner/name of this repo (one gh call) into RC_DEFAULT / RC_SLUG.
reconcile_repo_meta() {
  local out
  need gh
  out="$(GH_PROMPT_DISABLED=1 gh repo view --json nameWithOwner,defaultBranchRef \
    -q '.nameWithOwner + " " + .defaultBranchRef.name' </dev/null)" || return 1
  RC_SLUG="${out%% *}"
  RC_DEFAULT="${out#* }"
  [ -n "$RC_SLUG" ] && [ -n "$RC_DEFAULT" ] && [ "$RC_SLUG" != "$out" ]
}

# PRs merged into default branch $1 as TSV:
# number<TAB>headRefName<TAB>merged YYYY-MM-DD<TAB>,closing,issue,numbers,
# Closing issues count only when they live in repo $2 (owner/name). Fails on any
# gh/jq error so reconcile never acts on a partial list.
merged_prs_tsv() {
  local def="$1" slug="$2" json
  need gh
  need jq
  json="$(GH_PROMPT_DISABLED=1 gh pr list --state merged --base "$def" --limit "${FOAM_BOARD_PR_LIMIT:-200}" \
    --json number,headRefName,baseRefName,mergedAt,closingIssuesReferences </dev/null)" || return 1
  printf '%s' "$json" | jq -r --arg def "$def" --arg slug "$slug" '
    ($slug | ascii_downcase) as $s
    | .[] | select(.baseRefName == $def and (.mergedAt // "") != "")
    | [(.number | tostring), .headRefName, .mergedAt[0:10],
       ("," + ([.closingIssuesReferences[]?
                | select(((.repository.owner.login // "") + "/" + (.repository.name // "")
                          | ascii_downcase) == $s)
                | .number | tostring] | join(",")) + ",")] | @tsv'
}

# Drift rows (card<TAB>id<TAB>status<TAB>pr) for in-progress|in-review cards whose PR,
# listed in TSV file $1, merged on or after the card's `updated` day (the claim or
# in-review move; a PR merged before it is old work). A card's PR: head branch
# feat/<id>-issue-<id> (orch.sh), or a PR that closes the card's github_issue.
reconcile_drift() {
  local prs="$1" f st id gi upd pr
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    st="$(fm_get "$f" status)"
    case "$st" in in-progress|in-review) ;; *) continue ;; esac
    id="$(card_id "$f")"
    gi="$(fm_get "$f" github_issue)"
    upd="$(fm_get "$f" updated)"
    pr="$(awk -F'\t' -v b="feat/${id}-issue-${id}" -v gi="$gi" -v upd="$upd" \
      '$3 >= upd && ($2 == b || (gi != "" && index($4, "," gi ","))) { print $1; exit }' "$prs")"
    [ -n "$pr" ] || continue
    printf '%s\t%s\t%s\t%s\n' "$f" "$id" "$st" "$pr"
  done < <(card_files)
}

# True when the index of $1 (after `git add`) differs from $2 only in foam/board
# status/updated lines. The index, not the work tree: a new untracked card shows up.
reconcile_diff_is_status_only() {
  local wt="$1" base="$2" paths lines
  paths="$(git -C "$wt" diff --cached --name-only --no-renames "$base")" || return 1
  [ -n "$paths" ] || return 1
  [ -z "$(grep -v '^foam/board/' <<<"$paths" || true)" ] || return 1
  lines="$(git -C "$wt" diff --cached -U0 --no-renames "$base")" || return 1
  lines="$(grep -E '^[+-]' <<<"$lines" | grep -Ev '^(\+\+\+|---) ' || true)"
  [ -z "$(grep -Ev '^[+-](status|updated): ' <<<"$lines" || true)" ]
}

# cda-dev lib dir: sibling skill in the kit, else an installed copy.
cda_dev_lib() {
  local self d
  self="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  for d in "$self/../../cda-dev/scripts/lib" \
    "${HOME}/.claude/skills/cda-dev/scripts/lib" \
    "${HOME}/.cursor/skills/cda-dev/scripts/lib"; do
    if [ -f "$d/meta.sh" ]; then
      printf '%s\n' "$d"
      return 0
    fi
  done
  return 1
}

# What orch_merge_pr needs: default branch (worktree.sh), merge lock (lock.sh),
# verify (verify.sh), checks/mergeable (ci.sh), ADR-016 allowlist (meta.sh).
reconcile_load_merge_libs() {
  local lib f
  lib="$(cda_dev_lib)" || return 1
  for f in worktree.sh lock.sh verify.sh ci.sh meta.sh; do
    [ -f "$lib/$f" ] || return 1
    # shellcheck source=/dev/null
    source "$lib/$f" || return 1
  done
}

# One reconcile per repo. A dead owner pid, or a pid-less dir older than a minute (owner
# died between mkdir and the pid write), is stale and taken over.
reconcile_lock() {
  local lock="$1" owner
  if ! mkdir "$lock" 2>/dev/null; then
    owner="$(cat "$lock/pid" 2>/dev/null || true)"
    if [ -n "$owner" ]; then
      kill -0 "$owner" 2>/dev/null && return 1
    elif [ -z "$(find "$lock" -maxdepth 0 -mmin +1 2>/dev/null)" ]; then
      return 1
    fi
    rm -rf "$lock"
    mkdir "$lock" 2>/dev/null || return 1
  fi
  printf '%s\n' "$$" >"$lock/pid"
}

# Try once to land reconcile PR $4, checked out in $2 on branch $3. No CI wait: callers
# hold no run lock or pool slot on it; a PR left open is retried by the next reconcile.
# 0 = merged (then the done cards' open issues are closed), 3 = behind origin/$5 (the
# caller closes it and regenerates), 1 = left open.
reconcile_land() {
  local repo="$1" wt="$2" branch="$3" num="$4" def="$5" checks rc=0 f gi
  git -C "$wt" merge-base --is-ancestor "origin/$def" HEAD || return 3
  if grep -Eiq '^[[:space:]]*orch:[[:space:]]*allow-no-checks\b' "$wt/AGENTS.md" 2>/dev/null; then
    export ORCH_ALLOW_NO_CHECKS=1
  fi
  checks="$(orch_pr_checks_watch "$num" 0 2>/dev/null)" || true
  if [ "$checks" != green ]; then
    printf 'board: reconcile PR #%s checks %s; left open for the next reconcile\n' "$num" "${checks:-unknown}"
    return 1
  fi
  # ORCH_VERIFY=true: the diff is proven status/updated lines only; a repo verify adds
  # nothing. ORCH_CI_TIMEOUT=0: if a race makes it behind, fail now rather than watch CI.
  ORCH_DEFAULT_BRANCH="$def" ORCH_VERIFY=true ORCH_CI_TIMEOUT=0 \
    ORCH_MERGEABLE_WAIT="${FOAM_BOARD_MERGEABLE_WAIT:-20}" \
    ORCH_MERGE_LOCK_WAIT="${FOAM_BOARD_MERGE_LOCK_WAIT:-30}" \
    orch_meta_try_merge "$num" "$wt" "$branch" || rc=$?
  case "$rc" in
    0) printf 'board: merged reconcile PR #%s\n' "$num" ;;
    1) printf 'board: reconcile PR #%s outside the ADR-016 allowlist; left open\n' "$num"; return 1 ;;
    *) printf 'board: reconcile PR #%s not merged (%s); left open\n' "$num" "${ORCH_MERGE_ERROR:-unknown}"; return 1 ;;
  esac
  # Issues close only once the board says done on the default branch.
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    gi="$(fm_get "$wt/$f" github_issue)"
    [ -n "$gi" ] && [ "$(fm_get "$wt/$f" status)" = done ] || continue
    [ "$(GH_PROMPT_DISABLED=1 gh issue view "$gi" --json state -q .state </dev/null 2>/dev/null)" = OPEN ] || continue
    if GH_PROMPT_DISABLED=1 gh issue close "$gi" --comment "Done: card moved to done by board reconcile PR #$num." </dev/null >/dev/null; then
      printf 'board: closed issue #%s\n' "$gi"
    else
      err "could not close issue #$gi"
    fi
  done < <(git -C "$wt" diff --name-only "origin/$def" HEAD -- foam/board)
}

# Under the reconcile lock: land (or replace when behind) an open reconcile PR, else
# open one for the current drift. Subshell: `die` aborts only this pass.
reconcile_pr_locked() (
  local repo="$1" wt="$2/board" prs="$3" def="$4" open num head rc branch drift card id st pr body url
  git -C "$repo" fetch -q origin "$def" || die "git fetch origin $def failed"
  # Checked under the lock, so two reconciles never open duplicate PRs.
  open="$(GH_PROMPT_DISABLED=1 gh pr list --state open --limit 200 --json number,headRefName \
    -q '[.[] | select(.headRefName | startswith("board/reconcile-"))][0] // empty | "\(.number) \(.headRefName)"' \
    </dev/null)" || die "cannot list open PRs (gh)"
  if [ -n "$open" ]; then
    num="${open%% *}"
    head="${open#* }"
    git -C "$repo" fetch -q origin "+refs/heads/$head:refs/remotes/origin/$head" \
      && git -C "$repo" worktree add -q -B "$head" "$wt" "origin/$head" \
      || die "cannot check out reconcile PR #$num ($head)"
    rc=0
    reconcile_land "$repo" "$wt" "$head" "$num" "$def" || rc=$?
    [ "$rc" = 3 ] || exit 0
    git -C "$repo" worktree remove --force "$wt" || exit 1
    GH_PROMPT_DISABLED=1 gh pr close "$num" --delete-branch \
      --comment "Superseded: behind $def; the board reconcile regenerates it." </dev/null >/dev/null \
      || die "cannot close stale reconcile PR #$num"
    printf 'board: closed stale reconcile PR #%s (behind %s)\n' "$num" "$def"
  fi
  branch="board/reconcile-$(date -u +%Y%m%dT%H%M%SZ)-$$"
  git -C "$repo" worktree add -q -b "$branch" "$wt" "origin/$def" || die "cannot branch off origin/$def"
  OVR_ROOT="$wt"
  drift="$(reconcile_drift "$prs")"
  if [ -z "$drift" ]; then
    printf 'board: no drift\n'
    exit 0
  fi
  body="Cards whose PR is merged, moved to \`done\` by \`foam-board.sh reconcile --pr\` (ADR-016 §3b). Their GitHub issues close after this PR merges."$'\n\n'
  while IFS=$'\t' read -r card id st pr; do
    cmd_set_status "$card" done >/dev/null || exit 1
    printf 'drift %s %s -> done (PR #%s merged)\n' "$id" "$st" "$pr"
    body+="- \`${card#"$wt"/}\`: $st → done (#$pr)"$'\n'
  done <<<"$drift"
  git -C "$wt" add -A foam/board || exit 1
  reconcile_diff_is_status_only "$wt" "origin/$def" \
    || die "reconcile diff is not limited to foam/board status/updated lines; nothing pushed"
  git -C "$wt" commit -q -m "chore(foam): reconcile board with merged PRs" || exit 1
  git -C "$wt" push -q -u origin "$branch" || die "git push origin $branch failed"
  url="$(GH_PROMPT_DISABLED=1 gh pr create --base "$def" --head "$branch" \
    --title "chore(foam): reconcile board with merged PRs" --body "$body" </dev/null)" \
    || die "gh pr create failed for $branch"
  num="${url##*/}"
  printf 'board: opened PR #%s %s\n' "$num" "$url"
  reconcile_land "$repo" "$wt" "$branch" "$num" "$def" || true
)

# Throwaway worktrees off origin/<default>: the caller's checkout and the default
# branch are never written.
reconcile_pr() {
  local prs="$1" def="$2" repo common lock tmp b rc=0
  need git
  repo="$(repo_root)"
  reconcile_load_merge_libs || die "cda-dev scripts/lib not found (install the cda-dev skill)"
  export ORCH_REPO="$repo"
  common="$(cd "$repo" && cd "$(git rev-parse --git-common-dir)" && pwd)"
  lock="$common/foam-board-reconcile.lock"
  if ! reconcile_lock "$lock"; then
    printf 'board: another reconcile holds %s; skipped\n' "$lock"
    return 0
  fi
  tmp="$(mktemp -d)"
  reconcile_pr_locked "$repo" "$tmp" "$prs" "$def" || rc=$?
  git -C "$repo" worktree remove --force "$tmp/board" 2>/dev/null || true
  git -C "$repo" worktree prune 2>/dev/null || true
  for b in $(git -C "$repo" for-each-ref --format='%(refname:short)' refs/heads/board/); do
    case "$b" in board/reconcile-*) git -C "$repo" branch -q -D "$b" 2>/dev/null || true ;; esac
  done
  rm -rf "$tmp" "$lock"
  return "$rc"
}

# report (default) | write | pr
cmd_reconcile() {
  local mode="${1:-report}" prs drift card id st pr rc=0
  reconcile_repo_meta || die "cannot read the default branch (gh repo view); board unchanged"
  prs="$(mktemp)"
  if ! merged_prs_tsv "$RC_DEFAULT" "$RC_SLUG" >"$prs"; then
    rm -f "$prs"
    die "cannot list merged PRs (gh); board unchanged"
  fi
  if [ "$mode" = pr ]; then
    reconcile_pr "$prs" "$RC_DEFAULT" || rc=$?
    rm -f "$prs"
    return "$rc"
  fi
  drift="$(reconcile_drift "$prs")"
  rm -f "$prs"
  if [ -z "$drift" ]; then
    printf 'board: no drift\n'
    return 0
  fi
  while IFS=$'\t' read -r card id st pr; do
    printf 'drift %s %s -> done (PR #%s merged)\n' "$id" "$st" "$pr"
    if [ "$mode" = write ]; then
      cmd_set_status "$card" done >/dev/null
    fi
  done <<<"$drift"
}

main() {
  case "${1:-}" in
    -h|--help)
      sed -n '2,22p' "$0"
      exit 0
      ;;
  esac
  local sub="${1:-list-ready}"
  shift || true
  OVR_ROOT=""
  OUT_JSON=0
  local reconcile_mode=report
  local positional=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --root) OVR_ROOT="$2"; shift 2;;
      --repo) shift 2;; # orch-auto used to pass owner/repo; foam glob uses cwd/--root
      --json) OUT_JSON=1; shift;;
      --write) reconcile_mode=write; shift;;
      --pr) reconcile_mode=pr; shift;;
      --help|-h)
        sed -n '2,22p' "$0"
        exit 0
        ;;
      *) positional+=("$1"); shift;;
    esac
  done
  case "$sub" in
    list-ready) cmd_list_ready;;
    set-status) cmd_set_status "${positional[0]:-}" "${positional[1]:-}";;
    claim) cmd_claim "${positional[0]:-}";;
    path) cmd_path "${positional[0]:-}";;
    reconcile) cmd_reconcile "$reconcile_mode";;
    *) die "unknown subcommand: $sub (use: list-ready | set-status | claim | path | reconcile)";;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
