#!/usr/bin/env bash
# assemble-claude-md.sh — build claude/CLAUDE.md from header + sorted rules/*.md
#
# Usage:
#   ./scripts/assemble-claude-md.sh [--stdout] [OUTPUT_FILE]
#
# Default OUTPUT_FILE: claude/CLAUDE.md (repo root relative)
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HEADER="$REPO_DIR/claude/CLAUDE.header.md"
RULES_DIR="$REPO_DIR/rules"
STDOUT=0
OUTPUT="$REPO_DIR/claude/CLAUDE.md"

log() { printf '%s\n' "$*" >&2; }

assemble() {
  if [ ! -f "$HEADER" ]; then
    log "assemble-claude-md: missing $HEADER"
    exit 1
  fi
  cat "$HEADER"
  if [ -d "$RULES_DIR" ]; then
    local f
    for f in "$RULES_DIR"/*.md; do
      [ -f "$f" ] || continue
      printf '\n---\n\n'
      cat "$f"
    done
  fi
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --stdout) STDOUT=1;;
      -h|--help)
        sed -n '2,8p' "$0"
        exit 0
        ;;
      -*) log "unknown option: $1"; exit 2;;
      *) OUTPUT="$1";;
    esac
    shift
  done

  if [ "$STDOUT" = 1 ]; then
    assemble
    return
  fi

  run_dir="$(dirname "$OUTPUT")"
  mkdir -p "$run_dir"
  assemble > "$OUTPUT"
  log "wrote $OUTPUT"
}

main "$@"
