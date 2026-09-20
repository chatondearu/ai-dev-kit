#!/usr/bin/env bash
# new-plan.sh — create an implementation plan from PRD + Kanban issue.
#
# Usage:
#   ./new-plan.sh [--dry-run] [--force] [--status STATUS] ISSUE_NUMBER [slug]
#
# Requires: existing PRD for the issue (foam-prd), gh, plans/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../../foam-prd/scripts/_lib.sh
source "$SCRIPT_DIR/../../foam-prd/scripts/_lib.sh"

SKILL_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TEMPLATE="${FOAM_PLAN_TEMPLATE:-$SKILL_DIR/../foam-project-memory/templates/scaffold/plans/template.md}"
DRY=0
FORCE=0
STATUS="draft"
ISSUE=""
SLUG=""

log() { printf '%s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then printf 'DRY  %s\n' "$*"; else eval "$*"; fi; }
die() { printf 'new-plan.sh: %s\n' "$*" >&2; exit 1; }

usage() {
  sed -n '2,8p' "$0"
  exit 0
}

prefill_plan() {
  local dest="$1" title="$2" url="$3" num="$4" slug="$5" st="$6" prd_base="$7"
  local adr_table="" adr
  local kw
  kw="$(printf '%s' "$title" | tr '[:upper:]' '[:lower:]' | awk '{print $1,$2,$3}')"

  while IFS= read -r adr; do
    [ -n "$adr" ] || continue
    adr_table="${adr_table}| [[../foam/decisions/${adr}|${adr}]] | _(review)_ |\n"
  done < <(search_related_adrs "$ROOT" "$kw" || true)

  [ -n "$adr_table" ] || adr_table="| _(none matched — add ADRs manually)_ | … |\n"

  awk -v title="$title" -v url="$url" -v num="$num" -v slug="$slug" \
      -v date="$(today)" -v st="$st" -v prd="$prd_base" -v adrs="$adr_table" '
    BEGIN { n=0; inadr=0 }
    /^# PLAN-XXX/ { print "# PLAN-" num " — " title; next }
    /^- \*\*Status\*\*/ { print "- **Status**: " st; next }
    /^- \*\*PRD\*\*/ { print "- **PRD**: [[../prd/" prd "|" prd "]]"; next }
    /^- \*\*GitHub\*\*/ { print "- **GitHub**: [#" num "](" url ")"; next }
    /^- \*\*Date\*\*/ { print "- **Date**: " date; next }
    /^## Constraints \(from ADRs\)/ { print; print ""; print "| ADR | Constraint |"; print "| --- | ---------- |"; printf "%s", adrs; inadr=1; next }
    inadr && /^\| ADR \|/ { next }
    inadr && /^\| \[\[/ { next }
    inadr && /^Do not violate/ { inadr=0 }
    inadr && /^\| \[\[.*ADR-001/ { next }
  { print }
  ' "$dest" > "${dest}.tmp" && mv "${dest}.tmp" "$dest"
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) DRY=1;;
      --force) FORCE=1;;
      --status) STATUS="${2:?}"; shift;;
      -h|--help) usage;;
      -*) die "unknown option: $1";;
      *)
        if [ -z "$ISSUE" ]; then ISSUE="$1"
        else SLUG="$1"; fi
        ;;
    esac
    shift
  done

  [ -n "$ISSUE" ] || die "ISSUE_NUMBER required"
  [[ "$ISSUE" =~ ^[0-9]+$ ]] || die "ISSUE_NUMBER must be numeric"

  ROOT="$(repo_root)"
  [ -f "$TEMPLATE" ] || die "template not found: $TEMPLATE"
  [ -d "$ROOT/plans" ] || die "missing plans/ — run foam init first"

  PRD="$(find_prd_for_issue "$ISSUE" "$ROOT" || true)"
  [ -n "$PRD" ] || die "no PRD for issue #$ISSUE — run foam-prd/scripts/new-prd.sh first"

  DATA="$(issue_json "$ISSUE")" || die "cannot read issue #$ISSUE"
  TITLE="$(printf '%s' "$DATA" | jq -r .title)"
  URL="$(printf '%s' "$DATA" | jq -r .url)"
  PRD_BASE="$(basename "$PRD" .md)"

  [ -z "$SLUG" ] && SLUG="$(slugify "$TITLE")"
  DEST="$ROOT/plans/PLAN-${ISSUE}-${SLUG}.md"

  EXISTING="$(find_plan_for_issue "$ISSUE" "$ROOT" || true)"
  if [ -n "$EXISTING" ] && [ "$FORCE" != 1 ]; then
    log "ok   plan exists: $EXISTING (use --force to overwrite)"
    exit 0
  fi

  log "repo : $ROOT"
  log "issue: #$ISSUE"
  log "prd  : prd/$PRD_BASE.md"
  log "file : plans/$(basename "$DEST")"

  if [ "$DRY" = 1 ]; then
    log "DRY  would create $DEST"
    exit 0
  fi

  run "cp \"$TEMPLATE\" \"$DEST\""
  prefill_plan "$DEST" "$TITLE" "$URL" "$ISSUE" "$SLUG" "$STATUS" "$PRD_BASE"
  log "add  $DEST"
  log ""
  log "Next:"
  log "  1. Fill tasks, technical approach, test plan"
  log "  2. Verify ADR constraint table"
  log "  3. Kanban: claim issue → In progress (github-kanban-orchestrator workflow B)"
  log "  4. Implement against checklist; PR with Closes #$ISSUE"
}

main "$@"
