#!/usr/bin/env bash
# import-board.sh — one-shot map of open GitHub issues into foam/board cards.
#
# Not a loop. Run once when adopting foam-first kanban (ADR-016 / #408), then
# stop writing GitHub Projects. Existing cards are never overwritten.
#
# Usage:
#   ./import-board.sh [--dry-run] [--limit N] [TARGET_DIR]
#
# Requires: gh, jq. Each created card has github_issue set; status is backlog.
set -euo pipefail

DRY=0
LIMIT=500
TARGET=""

log() { printf '%s\n' "$*"; }
die() { printf 'import-board: %s\n' "$*" >&2; exit 1; }

usage() {
  sed -n '2,12p' "$0"
  exit 0
}

resolve_target() {
  if [ -n "$TARGET" ]; then
    cd "$TARGET" || die "cannot cd to $TARGET"
  fi
  if git rev-parse --show-toplevel >/dev/null 2>&1; then
    cd "$(git rev-parse --show-toplevel)"
  fi
  pwd
}

slugify() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-{2,}/-/g' \
    | cut -c1-40
}

# True when any card frontmatter has github_issue: N (filename may be {slug}.md).
card_exists_for_issue() {
  local n="$1" f base gi
  shopt -s nullglob
  for f in foam/board/*.md; do
    [ -f "$f" ] || continue
    base="$(basename "$f")"
    case "$base" in
      README.md|_*) continue ;;
    esac
    gi="$(awk '
      BEGIN { c=0 }
      /^---[[:space:]]*$/ { c++; if (c==2) exit; next }
      c==1 && $0 ~ /^[[:space:]]*#/ { next }
      c==1 && $0 ~ /^github_issue:/ {
        sub(/^github_issue:[[:space:]]*/, "")
        sub(/[[:space:]]+$/, "")
        print
        exit
      }
    ' "$f")"
    [ "$gi" = "$n" ] && return 0
  done
  return 1
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) DRY=1;;
      --limit) LIMIT="$2"; shift;;
      -h|--help) usage;;
      -*) die "unknown option: $1";;
      *) TARGET="$1";;
    esac
    shift
  done

  command -v gh >/dev/null 2>&1 || die "missing: gh"
  command -v jq >/dev/null 2>&1 || die "missing: jq"

  REPO_ROOT="$(resolve_target)"
  cd "$REPO_ROOT"
  [ -d foam ] || die "foam/ not found — run init.sh first"
  mkdir -p foam/board

  today="$(date -u +%Y-%m-%d)"
  json="$(gh issue list --state open --limit "$LIMIT" \
    --json number,title 2>/dev/null)" \
    || die "gh issue list failed"

  created=0
  skipped=0
  while IFS=$'\t' read -r num title; do
    [ -n "$num" ] || continue
    if card_exists_for_issue "$num"; then
      skipped=$((skipped + 1))
      log "skip #$num (card exists)"
      continue
    fi
    slug="$(slugify "$title")"
    [ -n "$slug" ] || slug="issue-$num"
    dest="foam/board/${num}-${slug}.md"
    if [ "$DRY" = 1 ]; then
      log "DRY  $dest"
      created=$((created + 1))
      continue
    fi
    # Escape title for YAML double quotes.
    ytitle="$(printf '%s' "$title" | sed 's/\\/\\\\/g; s/"/\\"/g')"
    cat >"$dest" <<EOF
---
title: "$ytitle"
status: backlog
priority: P2
updated: $today
github_issue: $num
---

One-shot import from \`gh issue list\`. Not a sync loop — edit this file for status.
EOF
    log "add  $dest"
    created=$((created + 1))
  done < <(printf '%s' "$json" | jq -r '.[] | "\(.number)\t\(.title | gsub("\n"; " "))"')

  log ""
  log "created $created, skipped $skipped (existing cards)."
  log "This is a one-shot map, not a poller. Status lives on the files."
}

main "$@"
