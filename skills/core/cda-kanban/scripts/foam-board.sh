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
# reconcile: an in-progress|in-review card whose PR is merged is drift.
# Default prints it; --write marks those cards done in the working tree;
# --pr does it on a branch off origin/<default>, opens a foam-only PR and
# self-merges it per ADR-016 (never commits on the default branch).
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

# Merged PRs as TSV: number<TAB>headRefName<TAB>,closing,issue,numbers,
# Fails (no output) on any gh/jq error so reconcile never acts on a partial list.
merged_prs_tsv() {
  local json
  need gh
  need jq
  json="$(GH_PROMPT_DISABLED=1 gh pr list --state merged --limit "${FOAM_BOARD_PR_LIMIT:-200}" \
    --json number,headRefName,closingIssuesReferences </dev/null)" || return 1
  printf '%s' "$json" | jq -r '.[] | [(.number | tostring), .headRefName,
    ("," + ([.closingIssuesReferences[]?.number | tostring] | join(",")) + ",")] | @tsv'
}

# Drift rows (card<TAB>id<TAB>status<TAB>pr) for in-progress|in-review cards whose PR,
# listed in TSV file $1, is merged. A card's PR: head branch feat/<id>-issue-<id>
# (orch.sh), or a PR that closes the card's github_issue.
reconcile_drift() {
  local prs="$1" f st id gi pr
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    st="$(fm_get "$f" status)"
    case "$st" in in-progress|in-review) ;; *) continue ;; esac
    id="$(card_id "$f")"
    gi="$(fm_get "$f" github_issue)"
    pr="$(awk -F'\t' -v b="feat/${id}-issue-${id}" -v gi="$gi" \
      '$2 == b || (gi != "" && index($3, "," gi ",")) { print $1; exit }' "$prs")"
    [ -n "$pr" ] || continue
    printf '%s\t%s\t%s\t%s\n' "$f" "$id" "$st" "$pr"
  done < <(card_files)
}

# True when the working tree of $1 differs from $2 only in foam/board status/updated lines.
reconcile_diff_is_status_only() {
  local wt="$1" base="$2" paths lines
  paths="$(git -C "$wt" diff --name-only --no-renames "$base")" || return 1
  [ -n "$paths" ] || return 1
  [ -z "$(grep -v '^foam/board/' <<<"$paths" || true)" ] || return 1
  lines="$(git -C "$wt" diff -U0 --no-renames "$base")" || return 1
  lines="$(grep -E '^[+-]' <<<"$lines" | grep -Ev '^(\+\+\+|---) ' || true)"
  [ -z "$(grep -Ev '^[+-](status|updated): ' <<<"$lines" || true)" ]
}

# cda-dev lib dir (ci.sh, meta.sh): sibling skill in the kit, else an installed copy.
cda_dev_lib() {
  local self d
  self="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  for d in "$self/../../cda-dev/scripts/lib" \
    "${HOME}/.claude/skills/cda-dev/scripts/lib" \
    "${HOME}/.cursor/skills/cda-dev/scripts/lib"; do
    if [ -f "$d/meta.sh" ] && [ -f "$d/ci.sh" ]; then
      printf '%s\n' "$d"
      return 0
    fi
  done
  return 1
}

# Apply drift in worktree $2 (branch $3 off origin/$4), open the PR, close done
# issues, self-merge per ADR-016. Subshell: `die` aborts only this step.
reconcile_pr_in() (
  local repo="$1" wt="$2" branch="$3" default="$4" prs="$5"
  local drift card id st pr gi body issues="" url num lib checks pair
  OVR_ROOT="$wt"
  drift="$(reconcile_drift "$prs")"
  if [ -z "$drift" ]; then
    printf 'board: no drift\n'
    exit 0
  fi
  body="Cards whose PR is merged, moved to \`done\` by \`foam-board.sh reconcile --pr\` (ADR-016)."$'\n\n'
  while IFS=$'\t' read -r card id st pr; do
    cmd_set_status "$card" done >/dev/null || exit 1
    printf 'drift %s %s -> done (PR #%s merged)\n' "$id" "$st" "$pr"
    body+="- \`${card#"$wt"/}\`: $st → done (#$pr)"$'\n'
    gi="$(fm_get "$card" github_issue)"
    [ -z "$gi" ] || issues+="$gi:$pr "
  done <<<"$drift"
  reconcile_diff_is_status_only "$wt" "origin/$default" \
    || die "reconcile diff is not limited to foam/board status/updated lines; nothing pushed"
  git -C "$wt" add -A foam/board || exit 1
  git -C "$wt" commit -q -m "chore(foam): reconcile board with merged PRs" || exit 1
  git -C "$wt" push -q -u origin "$branch" || die "git push origin $branch failed"
  url="$(GH_PROMPT_DISABLED=1 gh pr create --base "$default" --head "$branch" \
    --title "chore(foam): reconcile board with merged PRs" --body "$body" </dev/null)" \
    || die "gh pr create failed for $branch"
  num="${url##*/}"
  printf 'board: opened PR #%s %s\n' "$num" "$url"
  for pair in $issues; do
    gi="${pair%%:*}"
    [ "$(GH_PROMPT_DISABLED=1 gh issue view "$gi" --json state -q .state </dev/null 2>/dev/null)" = OPEN ] || continue
    GH_PROMPT_DISABLED=1 gh issue close "$gi" --comment "Done: PR #${pair#*:} merged (board reconcile #$num)." </dev/null >/dev/null \
      && printf 'board: closed issue #%s\n' "$gi" || err "could not close issue #$gi"
  done
  lib="$(cda_dev_lib)" || { err "cda-dev lib not found; PR #$num left open for the maintainer"; exit 0; }
  # shellcheck source=/dev/null
  source "$lib/ci.sh"
  # shellcheck source=/dev/null
  source "$lib/meta.sh"
  if grep -Eiq '^[[:space:]]*orch:[[:space:]]*allow-no-checks\b' "$wt/AGENTS.md" 2>/dev/null; then
    export ORCH_ALLOW_NO_CHECKS=1
  fi
  checks="$(orch_pr_checks_watch "$num" "${FOAM_BOARD_CHECKS_TIMEOUT:-600}")" || true
  if [ "$checks" != green ]; then
    err "PR #$num checks ${checks:-unknown}; left open for the maintainer"
    exit 0
  fi
  orch_meta_try_merge "$num" "$wt" || err "PR #$num left open for the maintainer"
)

# Branch off a fresh origin/<default> in a throwaway worktree; the caller's
# checkout and the default branch are never written. One reconcile at a time.
reconcile_pr() {
  local prs="$1" repo default num common lock owner tmp wt branch rc=0
  need git
  repo="$(repo_root)"
  default="$(git -C "$repo" symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null || true)"
  default="${default#origin/}"
  [ -n "$default" ] || default="$(GH_PROMPT_DISABLED=1 gh repo view --json defaultBranchRef \
    -q .defaultBranchRef.name </dev/null 2>/dev/null || true)"
  [ -n "$default" ] || die "cannot resolve the default branch of $repo"
  num="$(GH_PROMPT_DISABLED=1 gh pr list --state open --limit 200 --json number,headRefName \
    -q '[.[] | select(.headRefName | startswith("board/reconcile-"))][0].number // empty' </dev/null)" \
    || die "cannot list open PRs (gh)"
  if [ -n "$num" ]; then
    printf 'board: reconcile PR #%s already open; skipped\n' "$num"
    return 0
  fi
  common="$(cd "$repo" && cd "$(git rev-parse --git-common-dir)" && pwd)"
  lock="$common/foam-board-reconcile.lock"
  if ! mkdir "$lock" 2>/dev/null; then
    owner="$(cat "$lock/pid" 2>/dev/null || true)"
    if [ -z "$owner" ] || kill -0 "$owner" 2>/dev/null; then
      printf 'board: another reconcile holds %s; skipped\n' "$lock"
      return 0
    fi
    rm -rf "$lock"
    mkdir "$lock" 2>/dev/null || { printf 'board: another reconcile holds %s; skipped\n' "$lock"; return 0; }
  fi
  printf '%s\n' "$$" >"$lock/pid"
  tmp="$(mktemp -d)"
  wt="$tmp/board"
  branch="board/reconcile-$(date -u +%Y%m%dT%H%M%SZ)-$$"
  if git -C "$repo" fetch -q origin "$default" \
    && git -C "$repo" worktree add -q -b "$branch" "$wt" "origin/$default"; then
    reconcile_pr_in "$repo" "$wt" "$branch" "$default" "$prs" || rc=$?
    git -C "$repo" worktree remove --force "$wt" 2>/dev/null || true
    git -C "$repo" branch -q -D "$branch" 2>/dev/null || true
  else
    err "cannot branch off origin/$default"
    rc=1
  fi
  rm -rf "$tmp" "$lock"
  return "$rc"
}

# report (default) | write | pr
cmd_reconcile() {
  local mode="${1:-report}" prs drift card id st pr rc=0
  prs="$(mktemp)"
  if ! merged_prs_tsv >"$prs"; then
    rm -f "$prs"
    die "cannot list merged PRs (gh); board unchanged"
  fi
  if [ "$mode" = pr ]; then
    reconcile_pr "$prs" || rc=$?
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
      sed -n '2,21p' "$0"
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
        sed -n '2,21p' "$0"
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
