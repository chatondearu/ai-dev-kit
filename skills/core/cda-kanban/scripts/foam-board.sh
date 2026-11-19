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
#
# Requires: bash. jq only for --json. gh only for claim comments and for
# blocked_by numbers that do not match a foam card.
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

# Resolve id (github_issue, filename stem, or numeric prefix) to a card path.
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

main() {
  case "${1:-}" in
    -h|--help)
      sed -n '2,16p' "$0"
      exit 0
      ;;
  esac
  local sub="${1:-list-ready}"
  shift || true
  OVR_ROOT=""
  OUT_JSON=0
  local positional=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --root) OVR_ROOT="$2"; shift 2;;
      --repo) shift 2;; # orch-auto used to pass owner/repo; foam glob uses cwd/--root
      --json) OUT_JSON=1; shift;;
      --help|-h)
        sed -n '2,16p' "$0"
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
    *) die "unknown subcommand: $sub (use: list-ready | set-status | claim | path)";;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
