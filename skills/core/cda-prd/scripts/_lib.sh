#!/usr/bin/env bash
# _lib.sh — shared helpers for cda-prd / cda-plan scripts
set -euo pipefail

slugify() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+|-+$//g' \
    | cut -c1-60
}

repo_root() {
  if git rev-parse --show-toplevel >/dev/null 2>&1; then
    cd "$(git rev-parse --show-toplevel)"
  fi
  pwd
}

today() { date +%Y-%m-%d; }

issue_json() {
  local num="$1"
  gh issue view "$num" --json number,title,body,url,state 2>/dev/null
}

find_prd_for_issue() {
  local num="$1" root="$2"
  find "$root/prd" -maxdepth 1 -name "PRD-${num}-*.md" 2>/dev/null | head -1
}

find_plan_for_issue() {
  local num="$1" root="$2"
  find "$root/plans" -maxdepth 1 -name "PLAN-${num}-*.md" 2>/dev/null | head -1
}

append_prd_index_row() {
  local root="$1" prd_file="$2" status="${3:-draft}"
  local index="$root/foam/index.md"
  local base row
  base="$(basename "$prd_file" .md)"
  [ -f "$index" ] || return 0
  grep -qF "[[../prd/$base|" "$index" 2>/dev/null && return 0
  row="$(printf '| [[../prd/%s|%s]] | %s |' "$base" "$base" "$status")"
  # Insert right after the "| PRD |" table separator; drop the scaffold placeholder row.
  PRD_ROW="$row" awk '
    /^\| PRD \|/ { hdr=1 }
    /_\(none yet\)_/ && done { next }
    { print }
    hdr && !done && /^\| *-/ { print ENVIRON["PRD_ROW"]; done=1 }
  ' "$index" > "$index.tmp" && mv "$index.tmp" "$index"
  grep -qF "[[../prd/$base|" "$index" || printf '%s\n' "$row" >> "$index"
}

search_related_adrs() {
  local root="$1" keywords="$2"
  local adr_dir="$root/foam/decisions"
  [ -d "$adr_dir" ] || return 0
  local f
  for f in "$adr_dir"/ADR-*.md; do
    [ -f "$f" ] || continue
    if grep -qiE "$keywords" "$f" 2>/dev/null; then
      basename "$f" .md
    fi
  done
}
