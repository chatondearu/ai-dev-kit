#!/usr/bin/env bash
# doc-hygiene.sh — check foam / PRD / plan layout (cda-foam conventions).
#
# Usage:
#   doc-hygiene.sh [--check|--report] [--gh] [REPO_ROOT]
#   doc-hygiene.sh --archive-done [--dry-run] [REPO_ROOT]
#
# Exit: 0 if clean (check/report); non-zero on violations or tool error.
set -euo pipefail

MODE=check
DRY=0
WITH_GH=0
ROOT=""

log() { printf '%s\n' "$*"; }
fail_line() { printf 'FAIL: %s\n' "$*"; }

VIOLATIONS=0
WARNINGS=0

fail() {
  fail_line "$1"
  VIOLATIONS=$((VIOLATIONS + 1))
}

warn() {
  printf 'WARN: %s\n' "$1"
  WARNINGS=$((WARNINGS + 1))
}

usage() {
  sed -n '2,8p' "$0"
  exit 0
}

resolve_root() {
  if [ -n "$ROOT" ]; then
    cd "$ROOT" || exit 1
  elif git rev-parse --show-toplevel >/dev/null 2>&1; then
    cd "$(git rev-parse --show-toplevel)"
  fi
  pwd
}

plan_path_ok() {
  local rel="$1"
  case "$rel" in
    plans/PLAN-*|plans/template.md|plans/README.md) return 0 ;;
    foam/archive/plans/PLAN-*|foam/archive/plans/README.md) return 0 ;;
    skills/*/templates/*) return 0 ;;
    archive/*) return 0 ;;
  esac
  return 1
}

prd_path_ok() {
  local rel="$1"
  case "$rel" in
    prd/PRD-*|prd/template.md) return 0 ;;
    skills/*/templates/*) return 0 ;;
  esac
  return 1
}

should_skip_tree() {
  local rel="$1"
  case "$rel" in
    .git/*|.orch/*|node_modules/*|result/*|result-*/*|.direnv/*) return 0 ;;
  esac
  return 1
}

check_forbidden_dirs() {
  if [ -e "$REPO/docs/superpowers" ]; then
    fail "forbidden path exists: docs/superpowers (use foam/archive + prd/plans/doc per documentation-layout)"
  fi
  if git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if [ -n "$(git -C "$REPO" ls-files .superpowers 2>/dev/null || true)" ]; then
      fail "forbidden tracked files under .superpowers/ (remove from git; add to .gitignore)"
    fi
  fi
  if [ -d "$REPO/.superpowers" ]; then
    warn "local .superpowers/ present (should stay gitignored; rm -rf if stale SDD session)"
  fi
}

check_foam_baseline() {
  if [ ! -d "$REPO/foam" ]; then
    return 0
  fi
  [ -f "$REPO/foam/index.md" ] || fail "foam/ without foam/index.md"
  [ -f "$REPO/foam/product/documentation-layout.md" ] \
    || warn "foam/ missing foam/product/documentation-layout.md (copy from cda-foam scaffold)"
}

check_misplaced_plans() {
  local abs rel
  while IFS= read -r abs; do
    rel="${abs#"$REPO"/}"
    should_skip_tree "$rel" && continue
    plan_path_ok "$rel" || fail "misplaced plan: $rel"
  done < <(find "$REPO" -name 'PLAN-*.md' 2>/dev/null)
}

check_misplaced_prds() {
  local abs rel
  while IFS= read -r abs; do
    rel="${abs#"$REPO"/}"
    should_skip_tree "$rel" && continue
    prd_path_ok "$rel" || fail "misplaced PRD: $rel"
  done < <(find "$REPO" -name 'PRD-*.md' 2>/dev/null)
}

check_broken_plan_refs() {
  local f rel target alt base
  while IFS= read -r f; do
    rel="${f#"$REPO"/}"
    should_skip_tree "$rel" && continue
    while IFS= read -r target; do
      case "$target" in
        *'<'*|*'*'*|*'{'*|*NNN*|*XXX*|*slug.md) continue ;;
      esac
      [[ "$target" =~ plans/PLAN-[0-9] ]] || continue
      base="$(basename "$target")"
      alt="foam/archive/plans/$base"
      if [ -f "$REPO/$target" ] || [ -f "$REPO/$alt" ]; then
        continue
      fi
      fail "broken plan link in $rel → $target (not in plans/ or foam/archive/plans/)"
    done < <(grep -oE 'plans/PLAN-[^`)" ]+\.md' "$f" 2>/dev/null | sort -u || true)
  done < <(find "$REPO" -name '*.md' 2>/dev/null)
}

check_gh_archive_candidates() {
  [ "$WITH_GH" = 1 ] || return 0
  command -v gh >/dev/null 2>&1 || { warn "--gh: gh not on PATH"; return 0; }
  gh auth status >/dev/null 2>&1 || { warn "--gh: gh not authenticated"; return 0; }
  local remote owner repo plan issue state
  remote="$(git -C "$REPO" config --get remote.origin.url 2>/dev/null || true)"
  if [[ "$remote" =~ github\.com[:/]([^/]+)/([^/.]+)(\.git)?$ ]]; then
    owner="${BASH_REMATCH[1]}"
    repo="${BASH_REMATCH[2]}"
  else
    warn "--gh: cannot parse GitHub owner/repo from origin"
    return 0
  fi
  shopt -s nullglob
  for plan in "$REPO"/plans/PLAN-*.md; do
    [ -f "$plan" ] || continue
    issue="$(basename "$plan" | sed -n 's/^PLAN-\([0-9][0-9]*\)-.*/\1/p')"
    [ -n "$issue" ] || continue
    state="$(gh issue view "$issue" -R "$owner/$repo" --json state -q .state 2>/dev/null || true)"
    [ "$state" = "CLOSED" ] && warn "archive candidate: plans/$(basename "$plan") (issue #$issue closed)"
  done
  shopt -u nullglob
}

fix_plan_links_in_file() {
  local f="$1"
  sed -i \
    -e 's|\[\[../prd/|[[../../prd/|g' \
    -e 's|\[\[../foam/|[[../../foam/|g' \
    -e 's|](../prd/|](../../prd/|g' \
    -e 's|](../foam/|](../../foam/|g' \
    "$f"
}

run_archive_done() {
  command -v gh >/dev/null 2>&1 || { fail_line "archive-done requires gh on PATH"; exit 1; }
  gh auth status >/dev/null 2>&1 || { fail_line "archive-done requires gh auth"; exit 1; }
  local remote owner repo
  remote="$(git -C "$REPO" config --get remote.origin.url 2>/dev/null || true)"
  if [[ "$remote" =~ github\.com[:/]([^/]+)/([^/.]+)(\.git)?$ ]]; then
    owner="${BASH_REMATCH[1]}"
    repo="${BASH_REMATCH[2]}"
  else
    fail_line "cannot parse GitHub owner/repo from origin"
    exit 1
  fi
  mkdir -p "$REPO/foam/archive/plans"
  shopt -s nullglob
  for plan in "$REPO"/plans/PLAN-*.md; do
    [ -f "$plan" ] || continue
    local base issue state dest
    base="$(basename "$plan")"
    issue="$(sed -n 's/^PLAN-\([0-9][0-9]*\)-.*/\1/p' <<<"$base")"
    [ -n "$issue" ] || continue
    state="$(gh issue view "$issue" -R "$owner/$repo" --json state -q .state 2>/dev/null || true)"
    [ "$state" = "CLOSED" ] || continue
    dest="$REPO/foam/archive/plans/$base"
    if [ "$DRY" = 1 ]; then
      log "DRY  git mv plans/$base foam/archive/plans/$base"
    else
      git -C "$REPO" mv "plans/$base" "foam/archive/plans/$base"
      fix_plan_links_in_file "$dest"
      log "archived plans/$base → foam/archive/plans/$base"
    fi
  done
  shopt -u nullglob
}

run_checks() {
  check_forbidden_dirs
  check_foam_baseline
  check_misplaced_prds
  check_misplaced_plans
  check_broken_plan_refs
  check_gh_archive_candidates
}

print_report() {
  log "# Doc hygiene report"
  log ""
  log "Repo: $REPO"
  log "Mode: $MODE"
  log ""
  if [ "$VIOLATIONS" -gt 0 ]; then
    log "**Verdict:** FAIL ($VIOLATIONS violation(s), $WARNINGS warning(s))"
    log ""
    log "Fix violations, then re-run \`doc-hygiene.sh --check\`."
    log "Agent workflow: skill **cda-doc-hygiene**."
    return 1
  fi
  if [ "$WARNINGS" -gt 0 ]; then
    log "**Verdict:** OK with warnings ($WARNINGS)"
    log ""
    log "Optional: \`doc-hygiene.sh --archive-done --dry-run\` then fix cross-links in PRDs."
  else
    log "**Verdict:** OK"
  fi
  return 0
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --check) MODE=check ;;
      --report) MODE=report ;;
      --archive-done) MODE=archive-done ;;
      --dry-run) DRY=1 ;;
      --gh) WITH_GH=1 ;;
      -h|--help) usage ;;
      -*) fail_line "unknown option: $1"; exit 1 ;;
      *) ROOT="$1" ;;
    esac
    shift
  done

  REPO="$(resolve_root)"

  if [ "$MODE" = archive-done ]; then
    run_archive_done
    exit 0
  fi

  run_checks

  if [ "$MODE" = report ]; then
    print_report
    exit $((VIOLATIONS > 0 ? 1 : 0))
  fi

  if [ "$VIOLATIONS" -gt 0 ]; then
    log "doc-hygiene: $VIOLATIONS violation(s), $WARNINGS warning(s)" >&2
    exit 1
  fi
  if [ "$WARNINGS" -gt 0 ] && [ "${DOC_HYGIENE_STRICT:-0}" = 1 ]; then
    exit 1
  fi
  exit 0
}

main "$@"
