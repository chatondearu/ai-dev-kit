#!/usr/bin/env bash
# init.sh — scaffold foam project memory into a target git repository.
#
# Copies templates from ai-dev-kit (never commits — you git add/commit when ready).
#
# Usage:
#   ./init.sh [--dry-run] [--force] [TARGET_DIR]
#
# TARGET_DIR defaults to git rev-parse --show-toplevel or cwd.
set -euo pipefail

SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCAFFOLD="$SKILL_DIR/templates/scaffold"
DRY=0
FORCE=0
TARGET=""

log() { printf '%s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then printf 'DRY  %s\n' "$*"; else eval "$*"; fi; }

die() { printf 'init.sh: %s\n' "$*" >&2; exit 1; }

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

# Copy file or tree: src_rel → dest under repo root
install_path() {
  local src="$SCAFFOLD/$1"
  local dest="$REPO_ROOT/$1"
  if [ ! -e "$src" ]; then
    return 0
  fi
  if [ -e "$dest" ] || [ -L "$dest" ]; then
    if [ "$FORCE" = 1 ]; then
      run "rm -rf \"$dest\""
    else
      log "skip $1 (exists; use --force to overwrite)"
      return 0
    fi
  fi
  local parent
  parent="$(dirname "$dest")"
  run "mkdir -p \"$parent\""
  if [ -d "$src" ]; then
    run "cp -a \"$src\" \"$dest\""
  else
    run "cp \"$src\" \"$dest\""
  fi
  log "add  $1"
}

merge_agents_snippet() {
  local snippet="$SCAFFOLD/AGENTS.md.snippet"
  local agents="$REPO_ROOT/AGENTS.md"
  local marker="## Project memory"

  if [ ! -f "$agents" ]; then
    log "hint: no AGENTS.md — copy AGENTS.md.snippet section manually or create AGENTS.md"
    if [ "$DRY" = 1 ]; then
      log "DRY  would create AGENTS.md from snippet"
    elif [ "$FORCE" = 1 ]; then
      run "cp \"$snippet\" \"$agents\""
      log "add  AGENTS.md (from snippet)"
    else
      run "cp \"$snippet\" \"$agents\""
      log "add  AGENTS.md (from snippet)"
    fi
    return
  fi

  if grep -qF "$marker" "$agents" 2>/dev/null; then
    log "ok   AGENTS.md already has § Project memory"
    return
  fi

  if [ "$DRY" = 1 ]; then
    log "DRY  append AGENTS.md.snippet to AGENTS.md"
    return
  fi

  {
    printf '\n'
    cat "$snippet"
  } >> "$agents"
  log "append AGENTS.md § Project memory"
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) DRY=1;;
      --force) FORCE=1;;
      -h|--help) usage;;
      -*) die "unknown option: $1";;
      *) TARGET="$1";;
    esac
    shift
  done

  [ -d "$SCAFFOLD" ] || die "scaffold not found: $SCAFFOLD"

  REPO_ROOT="$(resolve_target)"
  log "repo : $REPO_ROOT"
  log "from : $SCAFFOLD"
  log ""

  # Directories and files (order matters for parents)
  install_path "foam"
  install_path "prd"
  install_path "plans"
  install_path ".foam"
  install_path ".vscode"
  install_path ".cursor"

  merge_agents_snippet

  log ""
  log "done."
  log "Next:"
  log "  1. Edit foam/product/vision.md"
  log "  2. bash $SKILL_DIR/scripts/import-kanban.sh   # if GitHub repo"
  log "  3. git add foam prd plans .foam .vscode .cursor AGENTS.md"
  log "  4. git commit -m \"docs: scaffold foam project memory\""
}

main "$@"
