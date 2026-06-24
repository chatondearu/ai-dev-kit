#!/usr/bin/env bash
# install.sh — link this kit's assets into one or more AI agents (portable, no Nix).
#
# Skills use the shared SKILL.md format, so the SAME canonical skills/ tree is
# linked into every detected agent. Only the install location differs per tool:
#   Cursor    ~/.cursor/skills
#   Claude    ~/.claude/skills
#   opencode  ${XDG_CONFIG_HOME:-~/.config}/opencode/skills
#   agents    ~/.agents/skills        (universal; opencode also reads this)
#
# Cursor-specific assets (user rules, subagents, local plugins) are linked into
# ~/.cursor only.
#
# Idempotent: re-running fixes/refreshes links. Existing non-symlink targets are
# backed up to <target>.bak-<timestamp> unless --force is given.
#
# Usage:
#   ./install.sh [--dry-run] [--force] [--uninstall]
#                [--cursor|--no-cursor] [--claude|--no-claude]
#                [--opencode|--no-opencode] [--agents|--no-agents]
#                [--cursor-home DIR]
#
# Tool selection: a tool defaults to ON when its config dir exists (Cursor is
# always considered). --<tool> forces ON, --no-<tool> forces OFF. The universal
# --agents target is OFF unless requested.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRY=0; FORCE=0; UNINSTALL=0
CURSOR_HOME="${CURSOR_HOME:-$HOME/.cursor}"
XDG="${XDG_CONFIG_HOME:-$HOME/.config}"

# tri-state per tool: "" = auto, 1 = on, 0 = off
declare -A WANT=([cursor]="" [claude]="" [opencode]="" [agents]="")

log() { printf '%s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then printf 'DRY  %s\n' "$*"; else eval "$*"; fi; }

link_one() {
  local src="$1" target="$2" parent
  parent="$(dirname "$target")"
  if [ -L "$target" ] && [ "$(readlink -f "$target")" = "$(readlink -f "$src")" ]; then
    log "ok   $target"; return
  fi
  run "mkdir -p \"$parent\""
  if [ -e "$target" ] || [ -L "$target" ]; then
    if [ "$FORCE" = 1 ]; then run "rm -rf \"$target\""
    else
      local bak="$target.bak-$(date +%Y%m%d%H%M%S)"
      log "back $target -> $bak"; run "mv \"$target\" \"$bak\""
    fi
  fi
  run "ln -s \"$src\" \"$target\""
  log "link $target -> $src"
}

unlink_one() {
  local src="$1" target="$2"
  if [ -L "$target" ] && [ "$(readlink -f "$target")" = "$(readlink -f "$src")" ]; then
    run "rm \"$target\""; log "rm   $target"
  fi
}

# link (or unlink) every immediate child of $1 into directory $2
apply_children() {
  local srcdir="$1" destdir="$2" path name
  [ -d "$srcdir" ] || return 0
  for path in "$srcdir"/*; do
    [ -e "$path" ] || continue
    name="$(basename "$path")"
    if [ "$UNINSTALL" = 1 ]; then unlink_one "$path" "$destdir/$name"
    else link_one "$path" "$destdir/$name"; fi
  done
}

skill_dir_for() {
  case "$1" in
    cursor) printf '%s/skills' "$CURSOR_HOME";;
    claude) printf '%s/.claude/skills' "$HOME";;
    opencode) printf '%s/opencode/skills' "$XDG";;
    agents) printf '%s/.agents/skills' "$HOME";;
  esac
}

is_enabled() {
  local tool="$1"
  case "${WANT[$tool]}" in
    1) return 0;; 0) return 1;;
  esac
  # auto
  case "$tool" in
    cursor) [ -d "$CURSOR_HOME" ] || [ ! -e "$CURSOR_HOME" ];;  # always
    claude) [ -d "$HOME/.claude" ];;
    opencode) [ -d "$XDG/opencode" ];;
    agents) return 1;;  # opt-in only
  esac
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) DRY=1;; --force) FORCE=1;; --uninstall) UNINSTALL=1;;
      --cursor) WANT[cursor]=1;; --no-cursor) WANT[cursor]=0;;
      --claude) WANT[claude]=1;; --no-claude) WANT[claude]=0;;
      --opencode) WANT[opencode]=1;; --no-opencode) WANT[opencode]=0;;
      --agents) WANT[agents]=1;; --no-agents) WANT[agents]=0;;
      --cursor-home) CURSOR_HOME="$2"; shift;;
      -h|--help) sed -n '2,28p' "$0"; exit 0;;
      *) log "unknown arg: $1"; exit 2;;
    esac
    shift
  done

  log "kit : $REPO_DIR"
  local tool enabled=()
  for tool in cursor claude opencode agents; do
    if is_enabled "$tool"; then enabled+=("$tool"); fi
  done
  log "tools: ${enabled[*]:-none}"
  log ""

  # Shared skills → every enabled tool
  for tool in "${enabled[@]}"; do
    log "# skills → $tool"
    apply_children "$REPO_DIR/skills" "$(skill_dir_for "$tool")"
  done

  # Cursor-specific assets
  if is_enabled cursor; then
    log "# cursor user-rules / agents / plugins"
    apply_children "$REPO_DIR/rules" "$CURSOR_HOME/user-rules"
    apply_children "$REPO_DIR/agents" "$CURSOR_HOME/agents"
    apply_children "$REPO_DIR/cursor/plugins/local" "$CURSOR_HOME/plugins/local"
  fi

  log ""
  if [ "$DRY" = 1 ]; then log "done (dry-run)"; else log "done"; fi
}

main "$@"
