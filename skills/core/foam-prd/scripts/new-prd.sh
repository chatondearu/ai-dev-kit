#!/usr/bin/env bash
# new-prd.sh — create or refresh a PRD from a GitHub Kanban issue.
#
# Usage:
#   ./new-prd.sh [--dry-run] [--force] [--status STATUS] ISSUE_NUMBER [slug]
#
# Requires: gh, git, foam scaffold (prd/, foam/)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=_lib.sh
source "$SCRIPT_DIR/_lib.sh"

SKILL_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TEMPLATE="${FOAM_PRD_TEMPLATE:-$SKILL_DIR/../foam-project-memory/templates/scaffold/prd/template.md}"
DRY=0
FORCE=0
STATUS="draft"
ISSUE=""
SLUG=""

log() { printf '%s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then printf 'DRY  %s\n' "$*"; else eval "$*"; fi; }
die() { printf 'new-prd.sh: %s\n' "$*" >&2; exit 1; }

usage() {
  sed -n '2,8p' "$0"
  exit 0
}

prefill_from_issue() {
  local dest="$1" title="$2" url="$3" body="$4" num="$5" slug="$6" st="$7"
  local problem
  problem="$(printf '%s' "$body" | sed -n '/^## Goal/,/^## /p' | head -n -1 | tail -n +2)"
  [ -n "$problem" ] || problem="$(printf '%s' "$body" | head -n 20)"

  awk -v title="$title" -v url="$url" -v num="$num" -v slug="$slug" \
      -v date="$(today)" -v st="$st" -v problem="$problem" '
    BEGIN { n=0 }
    /^# PRD-XXX/ { print "# PRD-" num " — " title; next }
    /^- \*\*Status\*\*/ { print "- **Status**: " st; next }
    /^- \*\*GitHub\*\*/ { print "- **GitHub**: [#" num "](" url ")"; next }
    /^- \*\*Date\*\*/ { print "- **Date**: " date; next }
    /^## Problem/ { print; inprob=1; next }
    inprob && /^## / { inprob=0 }
    inprob && NR>1 && problem != "" && !filled {
      print; print ""; print problem; filled=1; next
    }
    /^`plans\/PLAN-XXX/ {
      print "`plans/PLAN-" num "-" slug ".md` from `plans/template.md`"; next
    }
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
  [ -d "$ROOT/prd" ] || die "missing prd/ — run foam-project-memory init or setup-agents --init-foam"

  DATA="$(issue_json "$ISSUE")" || die "cannot read issue #$ISSUE (gh auth?)"
  TITLE="$(printf '%s' "$DATA" | jq -r .title)"
  BODY="$(printf '%s' "$DATA" | jq -r .body // ""')"
  URL="$(printf '%s' "$DATA" | jq -r .url)"

  [ -z "$SLUG" ] && SLUG="$(slugify "$TITLE")"
  DEST="$ROOT/prd/PRD-${ISSUE}-${SLUG}.md"

  EXISTING="$(find_prd_for_issue "$ISSUE" "$ROOT" || true)"
  if [ -n "$EXISTING" ] && [ "$FORCE" != 1 ]; then
    log "ok   PRD exists: $EXISTING (use --force to overwrite)"
    exit 0
  fi

  log "repo : $ROOT"
  log "issue: #$ISSUE $TITLE"
  log "file : prd/$(basename "$DEST")"

  if [ "$DRY" = 1 ]; then
    log "DRY  would create $DEST"
    exit 0
  fi

  run "cp \"$TEMPLATE\" \"$DEST\""
  prefill_from_issue "$DEST" "$TITLE" "$URL" "$BODY" "$ISSUE" "$SLUG" "$STATUS"
  append_prd_index_row "$ROOT" "$DEST" "$STATUS"
  log "add  $DEST"
  log ""
  log "Next:"
  log "  1. Complete PRD (goals, requirements, ADR links)"
  log "  2. Search foam/decisions/ and confront with ADRs"
  log "  3. foam-plan: bash .../foam-plan/scripts/new-plan.sh $ISSUE"
  log "  4. Kanban: move issue to Ready when PRD is approved (github-kanban-orchestrator)"
}

main "$@"
