#!/usr/bin/env bash
# kiss — Claude Code statusLine: "<model> · <dir> [KISS:<level>]".
# Reads the active level from the shared kiss flag. Fail-open: any missing piece
# just drops from the line.
input="$(cat 2>/dev/null)"
model="claude"; dir=""
if command -v jq >/dev/null 2>&1 && [ -n "$input" ]; then
  model="$(printf '%s' "$input" | jq -r '.model.display_name // .model.id // "claude"' 2>/dev/null)"
  dir="$(printf '%s' "$input" | jq -r '.workspace.current_dir // .cwd // ""' 2>/dev/null)"
fi
base="$(basename "$dir" 2>/dev/null)"

cfg="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
badge=""
if [ -f "$cfg/.kiss-active" ]; then
  mode="$(tr -d '[:space:]' < "$cfg/.kiss-active" 2>/dev/null)"
  case "$mode" in lite|full|ultra) badge=" [KISS:$mode]";; esac
fi

line="$model"
[ -n "$base" ] && line="$line · $base"
printf '%s%s' "$line" "$badge"
