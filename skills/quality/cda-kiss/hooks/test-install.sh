#!/usr/bin/env bash
# install.sh — hook-wiring test: drives the real CLI against temp config dirs only.
set -euo pipefail
cd "$(dirname "$0")/../../../.."   # repo root

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" CLAUDE_CONFIG_DIR="$tmp/claude" CURSOR_HOME="$tmp/cursor" XDG_CONFIG_HOME="$tmp/xdg"
mkdir -p "$HOME" "$CLAUDE_CONFIG_DIR" "$CURSOR_HOME"
S="$CLAUDE_CONFIG_DIR/settings.json" C="$CURSOR_HOME/hooks.json" K="$PWD/skills/quality/cda-kiss/hooks"

fail() { echo "FAIL: $*" >&2; exit 1; }
inst() { ./install.sh --claude --cursor --no-opencode "$@" >"$tmp/out" 2>&1 || { cat "$tmp/out" >&2; fail "install.sh $*"; }; }
count() { jq --arg k "$K" "[$2 | select(contains(\$k))] | length" "$1"; }
claude_cmds='.hooks[][].hooks[]?.command'
cursor_cmds='.hooks[][].command'

# pre-existing user config: a key + a foreign hook in each file
echo '{"model":"opus","hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo mine"}]}]}}' > "$S"
echo '{"version":1,"hooks":{"sessionStart":[{"command":"echo mine"}]}}' > "$C"

inst; inst   # second run must be a no-op
[ "$(jq -r .model "$S")" = opus ] || fail "model key lost"
jq -e '[.hooks.SessionStart[].hooks[].command] | index("echo mine")' "$S" >/dev/null || fail "foreign claude hook lost"
jq -e '[.hooks.sessionStart[].command] | index("echo mine")' "$C" >/dev/null || fail "foreign cursor hook lost"
[ "$(count "$S" "$claude_cmds")" = 3 ] || fail "claude kiss hooks not exactly once"
[ "$(count "$C" "$cursor_cmds")" = 2 ] || fail "cursor kiss hooks not exactly once"
jq -e --arg k "$K" '.statusLine.command | contains($k)' "$S" >/dev/null || fail "kiss statusLine not set"
[ "$(readlink "$CLAUDE_CONFIG_DIR/CLAUDE.md")" = "$PWD/claude/CLAUDE.md" ] || fail "CLAUDE.md not linked into CLAUDE_CONFIG_DIR"
[ ! -e "$HOME/.claude/CLAUDE.md" ] || fail "CLAUDE.md linked into \$HOME/.claude"

# uninstall: only kiss entries + kiss-owned statusLine go
inst --uninstall
[ "$(count "$S" "$claude_cmds")" = 0 ] && [ "$(count "$C" "$cursor_cmds")" = 0 ] || fail "kiss hooks left after uninstall"
jq -e '.statusLine == null and .model == "opus"' "$S" >/dev/null || fail "uninstall statusLine/model wrong"
jq -e '.hooks.SessionStart | length == 1' "$S" >/dev/null || fail "foreign claude hook removed"
jq -e '.hooks.sessionStart | length == 1' "$C" >/dev/null || fail "foreign cursor hook removed"

# a foreign statusLine is kept on install and on uninstall
jq '.statusLine = {type:"command", command:"my-line"}' "$S" > "$tmp/s" && mv "$tmp/s" "$S"
inst;              [ "$(jq -r .statusLine.command "$S")" = my-line ] || fail "foreign statusLine clobbered"
inst --uninstall;  [ "$(jq -r .statusLine.command "$S")" = my-line ] || fail "foreign statusLine removed"

# invalid JSON: warned by name, left byte-identical
printf '{ not json\n' > "$S"; cp "$S" "$tmp/orig"
inst
grep -q "warn: $S" "$tmp/out" || fail "no warning naming $S"
cmp -s "$S" "$tmp/orig" || fail "invalid settings.json modified"
inst --uninstall
grep -q "warn: $S" "$tmp/out" || fail "no warning on uninstall"
cmp -s "$S" "$tmp/orig" || fail "invalid settings.json modified by uninstall"

echo "ok: install.sh hook wiring"
