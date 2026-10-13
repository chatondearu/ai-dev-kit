#!/usr/bin/env bash
# install.sh — link this kit's assets into one or more AI agents (portable, no Nix).
#
# Skills use the shared SKILL.md format, so the SAME canonical skills/ tree is
# linked into every detected agent. Only the install location differs per tool.
# Each tool's root honors its own env var first, then the conventional default:
#   Cursor    ${CURSOR_HOME:-~/.cursor}/skills
#   Claude    ${CLAUDE_CONFIG_DIR:-~/.claude}/skills
#   opencode  ${XDG_CONFIG_HOME:-~/.config}/opencode/skills
#   agents    ${AGENTS_HOME:-~/.agents}/skills   (universal; opencode also reads this)
#
# Cursor-specific assets (user rules, subagents, local plugins) → ~/.cursor only.
# Claude: assembled claude/CLAUDE.md → ~/.claude/CLAUDE.md
# Skills may live in nested folders (skills/core/foo/); installers flatten by
# skill directory name (each dir containing SKILL.md).
#
# Idempotent: re-running fixes/refreshes links. Existing non-symlink targets are
# backed up to <target>.bak-<timestamp> unless --force is given.
#
# Usage:
#   ./install.sh [--dry-run] [--force] [--uninstall] [--no-hooks]
#                [--cursor|--no-cursor] [--claude|--no-claude]
#                [--opencode|--no-opencode] [--agents|--no-agents]
#                [--cursor-home DIR]
#
# Hooks: unless --no-hooks, the kiss code-attitude ruleset is wired into each
# enabled tool's lifecycle hooks (Claude settings.json, Cursor hooks.json) so it
# is injected every session + into subagents. Needs jq; merge is idempotent.
#
# Tool selection: a tool defaults to ON when its config dir exists (Cursor is
# always considered). --<tool> forces ON, --no-<tool> forces OFF. The universal
# --agents target is OFF unless requested.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRY=0; FORCE=0; UNINSTALL=0; HOOKS=1
# Per-tool install roots: honor each tool's own env var first, fall back to the
# conventional default. XDG is only used where the tool natively reads it
# (opencode); Claude/Cursor don't follow XDG, so we don't force it on them.
CURSOR_HOME="${CURSOR_HOME:-$HOME/.cursor}"
XDG="${XDG_CONFIG_HOME:-$HOME/.config}"
CLAUDE_HOME="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
AGENTS_HOME="${AGENTS_HOME:-$HOME/.agents}"

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

# Link every directory under $srcdir that contains SKILL.md (flattened by name).
link_skill_tree() {
  local srcdir="$1" destdir="$2" skill_md skill_dir skill_name
  [ -d "$srcdir" ] || return 0
  while IFS= read -r skill_md; do
    [ -n "$skill_md" ] || continue
    skill_dir="$(dirname "$skill_md")"
    skill_name="$(basename "$skill_dir")"
    if [ "$UNINSTALL" = 1 ]; then unlink_one "$skill_dir" "$destdir/$skill_name"
    else link_one "$skill_dir" "$destdir/$skill_name"; fi
  done < <(find "$srcdir" -name SKILL.md -type f 2>/dev/null | sort)
}

assemble_claude_md() {
  local script="$REPO_DIR/scripts/assemble-claude-md.sh"
  [ -x "$script" ] || chmod +x "$script" 2>/dev/null || true
  if [ "$DRY" = 1 ]; then
    log "DRY  $script"
    return
  fi
  bash "$script"
}

skill_dir_for() {
  case "$1" in
    cursor) printf '%s/skills' "$CURSOR_HOME";;
    claude) printf '%s/skills' "$CLAUDE_HOME";;
    opencode) printf '%s/opencode/skills' "$XDG";;
    agents) printf '%s/skills' "$AGENTS_HOME";;
  esac
}

# --- kiss code-attitude hooks -------------------------------------------------
# Wire the always-on KISS ruleset into a tool's lifecycle hooks (merge, never
# clobber; idempotent; needs jq). Claude: settings.json SessionStart/Subagent/
# UserPromptSubmit. Cursor: hooks.json sessionStart/beforeSubmitPrompt.
KISS_HOOKS_DIR="$REPO_DIR/skills/quality/kiss/hooks"

_kiss_jq_inplace() { # $1=file  $2=jq-program  (rest: --arg pairs already in program via env)
  local file="$1" prog="$2"; shift 2
  local tmp="$file.kiss.tmp"
  if jq "$@" "$prog" "$file" > "$tmp" 2>/dev/null; then mv "$tmp" "$file"; else rm -f "$tmp"; return 1; fi
}

wire_claude_hooks() {
  local f="$CLAUDE_HOME/settings.json" dir="$KISS_HOOKS_DIR"
  command -v jq >/dev/null 2>&1 || { log "skip kiss hooks (claude): jq not found"; return 0; }
  if [ "$DRY" = 1 ]; then log "DRY  wire kiss hooks → $f"; return 0; fi
  mkdir -p "$CLAUDE_HOME"; [ -f "$f" ] || printf '{}\n' > "$f"
  _kiss_jq_inplace "$f" '
    def strip(ev): (.hooks[ev] // []) | map(select(any(.hooks[]?; .command | contains($dir)) | not));
    .hooks //= {}
    | .hooks.SessionStart     = (strip("SessionStart")     + [{matcher:"startup|resume|clear|compact", hooks:[{type:"command", command:$act, timeout:5}]}])
    | .hooks.SubagentStart    = (strip("SubagentStart")    + [{hooks:[{type:"command", command:$sub, timeout:5}]}])
    | .hooks.UserPromptSubmit = (strip("UserPromptSubmit") + [{hooks:[{type:"command", command:$trk, timeout:5}]}])
  ' --arg dir "$dir" --arg act "node \"$dir/activate.js\"" --arg sub "node \"$dir/subagent.js\"" --arg trk "node \"$dir/mode-tracker.js\"" \
    && log "hook $f (kiss: SessionStart/SubagentStart/UserPromptSubmit)"
  # statusLine badge — set ONLY when the user has none (never clobber theirs).
  local sl="bash \"$dir/kiss-statusline.sh\""
  if jq -e '.statusLine == null' "$f" >/dev/null 2>&1; then
    _kiss_jq_inplace "$f" '.statusLine = {type:"command", command:$sl}' --arg sl "$sl" \
      && log "statusline $f (kiss badge set)"
  elif ! jq -e --arg d "$dir" '(.statusLine.command? // "") | contains($d)' "$f" >/dev/null 2>&1; then
    log "note: existing statusLine kept — add [KISS] badge by hand if you want it"
  fi
}

wire_cursor_hooks() {
  local f="$CURSOR_HOME/hooks.json" dir="$KISS_HOOKS_DIR"
  command -v jq >/dev/null 2>&1 || { log "skip kiss hooks (cursor): jq not found"; return 0; }
  if [ "$DRY" = 1 ]; then log "DRY  wire kiss hooks → $f"; return 0; fi
  mkdir -p "$CURSOR_HOME"; [ -f "$f" ] || printf '{"version":1,"hooks":{}}\n' > "$f"
  _kiss_jq_inplace "$f" '
    def strip(ev): (.hooks[ev] // []) | map(select(.command | contains($dir) | not));
    .version //= 1 | .hooks //= {}
    | .hooks.sessionStart       = (strip("sessionStart")       + [{command:$act, timeout:5}])
    | .hooks.beforeSubmitPrompt = (strip("beforeSubmitPrompt") + [{command:$trk, timeout:5}])
  ' --arg dir "$dir" --arg act "node \"$dir/activate.js\"" --arg trk "node \"$dir/mode-tracker.js\"" \
    && log "hook $f (kiss: sessionStart/beforeSubmitPrompt)"
}

unwire_hooks() { # strip kiss entries from a tool's hook file, drop now-empty events
  local f="$1" dir="$KISS_HOOKS_DIR"
  command -v jq >/dev/null 2>&1 || return 0
  [ -f "$f" ] || return 0
  if [ "$DRY" = 1 ]; then log "DRY  unwire kiss hooks ← $f"; return 0; fi
  _kiss_jq_inplace "$f" '
    .hooks //= {}
    | .hooks |= with_entries(.value |= map(select(
        (.command? // "" | contains($dir)) or (any(.hooks[]?; .command | contains($dir))) | not)))
    | .hooks |= with_entries(select(.value | length > 0))
    | if (.statusLine.command? // "") | contains($dir) then del(.statusLine) else . end
  ' --arg dir "$dir" && log "unhook $f (kiss removed)"
}

is_enabled() {
  local tool="$1"
  case "${WANT[$tool]}" in
    1) return 0;; 0) return 1;;
  esac
  # auto
  case "$tool" in
    cursor) [ -d "$CURSOR_HOME" ] || [ ! -e "$CURSOR_HOME" ];;  # always
    claude) [ -d "$CLAUDE_HOME" ];;
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
      --hooks) HOOKS=1;; --no-hooks) HOOKS=0;;
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

  # Shared skills → every enabled tool (nested categories flattened)
  for tool in "${enabled[@]}"; do
    log "# skills → $tool"
    link_skill_tree "$REPO_DIR/skills" "$(skill_dir_for "$tool")"
  done

  # Cursor-specific assets
  if is_enabled cursor; then
    log "# cursor user-rules / agents / plugins"
    apply_children "$REPO_DIR/rules" "$CURSOR_HOME/user-rules"
    apply_children "$REPO_DIR/agents" "$CURSOR_HOME/agents"
    apply_children "$REPO_DIR/cursor/plugins/local" "$CURSOR_HOME/plugins/local"
  fi

  # Claude Code — global instructions from rules/
  if is_enabled claude; then
    log "# claude CLAUDE.md (from rules/)"
    assemble_claude_md
    link_one "$REPO_DIR/claude/CLAUDE.md" "$HOME/.claude/CLAUDE.md"
  fi

  # kiss code-attitude hooks → enabled tools (SessionStart inject + mode toggle)
  if [ "$HOOKS" = 1 ]; then
    if is_enabled claude; then
      log "# kiss hooks → claude"
      if [ "$UNINSTALL" = 1 ]; then unwire_hooks "$CLAUDE_HOME/settings.json"; else wire_claude_hooks; fi
    fi
    if is_enabled cursor; then
      log "# kiss hooks → cursor"
      if [ "$UNINSTALL" = 1 ]; then unwire_hooks "$CURSOR_HOME/hooks.json"; else wire_cursor_hooks; fi
    fi
    if is_enabled opencode; then
      log "# kiss plugin → opencode"
      local mjs="$REPO_DIR/skills/quality/kiss/opencode/kiss.mjs"
      if [ "$UNINSTALL" = 1 ]; then unlink_one "$mjs" "$XDG/opencode/plugin/kiss.mjs"
      else link_one "$mjs" "$XDG/opencode/plugin/kiss.mjs"; fi
    fi
  fi

  log ""
  if [ "$DRY" = 1 ]; then log "done (dry-run)"; else log "done"; fi
}

main "$@"
