#!/usr/bin/env bash
# install.sh — link this kit's Cursor assets into ~/.cursor (portable, no Nix).
#
# Source of truth = this repo's `cursor/` tree. Each immediate child of
#   cursor/skills, cursor/user-rules, cursor/agents, cursor/plugins/local
# is symlinked to the matching path under $CURSOR_HOME (default ~/.cursor).
#
# Idempotent: re-running fixes/refreshes links. Existing non-symlink targets
# are backed up to <target>.bak-<timestamp> unless --force is given.
#
# Usage:
#   ./install.sh [--dry-run] [--force] [--uninstall] [--cursor-home DIR]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CURSOR_SRC="$REPO_DIR/cursor"
DEST="${CURSOR_HOME:-$HOME/.cursor}"
DRY=0; FORCE=0; UNINSTALL=0

log() { printf '%s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then printf 'DRY  %s\n' "$*"; else eval "$*"; fi; }

# Parent directories whose children get linked individually (so other tools'
# files living next to ours are never clobbered).
SUBDIRS=(skills user-rules agents plugins/local)

link_one() {
  local src="$1" target="$2"
  local parent; parent="$(dirname "$target")"

  if [ -L "$target" ] && [ "$(readlink -f "$target")" = "$(readlink -f "$src")" ]; then
    log "ok   $target"
    return
  fi

  run "mkdir -p \"$parent\""

  if [ -e "$target" ] || [ -L "$target" ]; then
    if [ "$FORCE" = 1 ]; then
      run "rm -rf \"$target\""
    else
      local bak="$target.bak-$(date +%Y%m%d%H%M%S)"
      log "back $target -> $bak"
      run "mv \"$target\" \"$bak\""
    fi
  fi

  run "ln -s \"$src\" \"$target\""
  log "link $target -> $src"
}

unlink_one() {
  local src="$1" target="$2"
  if [ -L "$target" ] && [ "$(readlink -f "$target")" = "$(readlink -f "$src")" ]; then
    run "rm \"$target\""
    log "rm   $target"
  fi
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) DRY=1; shift;;
      --force) FORCE=1; shift;;
      --uninstall) UNINSTALL=1; shift;;
      --cursor-home) DEST="$2"; shift 2;;
      -h|--help) sed -n '2,12p' "$0"; exit 0;;
      *) log "unknown arg: $1"; exit 2;;
    esac
  done

  [ -d "$CURSOR_SRC" ] || { log "missing $CURSOR_SRC"; exit 1; }
  log "kit : $REPO_DIR"
  log "dest: $DEST"
  log ""

  local sub path name target
  for sub in "${SUBDIRS[@]}"; do
    [ -d "$CURSOR_SRC/$sub" ] || continue
    for path in "$CURSOR_SRC/$sub"/*; do
      [ -e "$path" ] || continue
      name="$(basename "$path")"
      target="$DEST/$sub/$name"
      if [ "$UNINSTALL" = 1 ]; then
        unlink_one "$path" "$target"
      else
        link_one "$path" "$target"
      fi
    done
  done

  log ""
  if [ "$DRY" = 1 ]; then log "done (dry-run)"; else log "done"; fi
}

main "$@"
