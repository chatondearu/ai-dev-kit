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

gh_repo_url() {
  gh repo view --json url -q .url 2>/dev/null || printf 'https://github.com/OWNER/REPO'
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
  local base
  base="$(basename "$prd_file" .md)"
  [ -f "$index" ] || return 0
  if grep -qF "| $base |" "$index" 2>/dev/null; then
    return 0
  fi
  if grep -q '_| _(none yet)_' "$index" 2>/dev/null; then
    sed -i "s/| _(none yet)_ |.*/| [[../prd\/$base|$base]] | $status |/" "$index"
  else
    printf '| [[../prd/%s|%s]] | %s |\n' "$base" "$base" "$status" >> "$index"
  fi
}

append_plan_index_hint() {
  local root="$1" plan_file="$2"
  local index="$root/foam/index.md"
  local base
  base="$(basename "$plan_file" .md)"
  [ -f "$index" ] || return 0
  grep -qF "$base" "$index" 2>/dev/null && return 0
  # Append under PRDs & plans section if a plans list is added later
  return 0
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
