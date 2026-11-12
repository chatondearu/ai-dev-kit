#!/usr/bin/env bash
# cdamux-hooks wire|unwire in packaged mode (the Nix layout: a `node` next to the
# script), driven against temp config dirs only — never the live config.
set -euo pipefail
cd "$(dirname "$0")"
CHECKOUT="$PWD"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" CLAUDE_CONFIG_DIR="$tmp/claude" CURSOR_HOME="$tmp/cursor" \
  XDG_CONFIG_HOME="$tmp/xdg" XDG_DATA_HOME="$tmp/data" CODEX_HOME="$tmp/codex"
mkdir -p "$HOME" "$CLAUDE_CONFIG_DIR" "$CURSOR_HOME" "$XDG_CONFIG_HOME/opencode" "$CODEX_HOME"
S="$CLAUDE_CONFIG_DIR/settings.json" C="$CURSOR_HOME/hooks.json"
OCL="$XDG_CONFIG_HOME/opencode/plugins/cdamux-subagent.js"
STABLE="$XDG_DATA_HOME/cdamux/hooks"

# fake store dir: the package layout (hooks + script + node link)
PKG="$tmp/store/share/cdamux/hooks"
mkdir -p "$PKG"; cp ./*.js cdamux-hooks "$PKG/"; ln -s "$(command -v node)" "$PKG/node"

fail() { echo "FAIL: $*" >&2; exit 1; }
wire() { bash "$PKG/cdamux-hooks" "$@" >"$tmp/out" 2>&1 || { cat "$tmp/out" >&2; fail "cdamux-hooks $*"; }; }
claude_cmds='[.hooks[][].hooks[]? | (.command // empty)]'
cursor_cmds='[.hooks[][].command]'
n() { jq --arg m "$2" "$3 | map(select(contains(\$m))) | length" "$1"; }

# existing config: kiss + foreign + an old checkout cdamux entry in each file
K="/kit/skills/quality/cda-kiss/hooks/activate.js" OLD="node \"/old/kit/cdamux/hooks/claude-subagent.js\""
jq -n --arg k "node \"$K\"" --arg old "$OLD" '{model:"opus", hooks:{
  SessionStart:[{hooks:[{type:"command",command:"echo mine"}]},{hooks:[{type:"command",command:$k}]}],
  SubagentStart:[{hooks:[{type:"command",command:$old}]}],
  Stop:[{hooks:[{type:"prompt",prompt:"keep"}]}]}}' > "$S"
jq -n --arg k "node \"$K\"" --arg old "$OLD" '{version:1, hooks:{
  sessionStart:[{command:"echo mine"},{command:$k}], subagentStop:[{command:$old}]}}' > "$C"
mkdir -p "$(dirname "$OCL")"; ln -s "/old/kit/cdamux/hooks/opencode-subagent.js" "$OCL"

wire wire; cp "$S" "$tmp/s1"; cp "$C" "$tmp/c1"
wire wire
cmp -s "$S" "$tmp/s1" && cmp -s "$C" "$tmp/c1" || fail "second wire is not a no-op"
[ "$(readlink "$STABLE")" = "$PKG" ] || fail "stable link not pointing at the package dir"
[ "$(readlink "$OCL")" = "$STABLE/opencode-subagent.js" ] || fail "opencode plugin not relinked to the stable path"

[ "$(n "$S" /old/kit/ "$claude_cmds")" = 0 ] && [ "$(n "$C" /old/kit/ "$cursor_cmds")" = 0 ] || fail "old checkout entries kept"
[ "$(n "$S" "$STABLE/" "$claude_cmds")" = 7 ] || fail "claude: expected 7 cdamux entries on the stable path"
[ "$(n "$C" "$STABLE/" "$cursor_cmds")" = 6 ] || fail "cursor: expected 6 cdamux entries on the stable path"
CX="$CODEX_HOME/hooks.json"
[ "$(n "$CX" "$STABLE/" "$claude_cmds")" = 5 ] || fail "codex: expected 5 cdamux entries on the stable path"
[ "$(n "$S" "$tmp/store" "$claude_cmds")" = 0 ] && [ "$(n "$C" "$tmp/store" "$cursor_cmds")" = 0 ] || fail "store path written into settings"
[ "$(n "$S" cda-kiss "$claude_cmds")" = 1 ] && [ "$(n "$C" cda-kiss "$cursor_cmds")" = 1 ] || fail "kiss entry changed"
[ "$(n "$S" "echo mine" "$claude_cmds")" = 1 ] && [ "$(n "$C" "echo mine" "$cursor_cmds")" = 1 ] || fail "foreign entry lost"
jq -e '.model == "opus" and .hooks.Stop[0].hooks[0].prompt == "keep"' "$S" >/dev/null || fail "other settings lost"

# the written command runs with the packaged node (outside tmux: no-op, exit 0)
ss="$(jq -r '.hooks.SessionStart[-1].hooks[0].command' "$S")"
case "$ss" in "\"$STABLE/node\" \"$STABLE/session-start.js\"") ;; *) fail "unexpected command: $ss";; esac
printf '{"hook_event_name":"SessionStart","session_id":"x"}' | env -u TMUX PATH=/usr/bin:/bin sh -c "$ss" || fail "hook command failed"

# no target: a tool without a config dir is skipped
rm -rf "$CURSOR_HOME"; wire wire; [ ! -e "$CURSOR_HOME" ] || fail "cursor config dir created"; mkdir -p "$CURSOR_HOME"; cp "$tmp/c1" "$C"

wire unwire
[ "$(n "$S" /cdamux/hooks/ "$claude_cmds")" = 0 ] && [ "$(n "$C" /cdamux/hooks/ "$cursor_cmds")" = 0 ] || fail "cdamux entries left after unwire"
jq -e '.hooks.SubagentStart == null and .hooks.SubagentStop == null' "$S" >/dev/null || fail "empty events left"
[ "$(n "$S" cda-kiss "$claude_cmds")" = 1 ] && [ "$(n "$C" "echo mine" "$cursor_cmds")" = 1 ] || fail "unwire touched kiss/foreign"
[ ! -L "$OCL" ] || fail "opencode plugin link left after unwire"

# checkout mode (no `node` next to the script): plain `node "<checkout>/x.js"`
bash "$CHECKOUT/cdamux-hooks" wire claude >/dev/null
jq -e --arg c "node \"$CHECKOUT/claude-subagent.js\"" '[.hooks.SubagentStart[].hooks[].command] | index($c)' "$S" >/dev/null \
  || fail "checkout command wrong"
[ "$(n "$S" "$STABLE/" "$claude_cmds")" = 0 ] || fail "checkout wire kept stable entries"

! bash "$PKG/cdamux-hooks" bogus >/dev/null 2>&1 || fail "unknown action accepted"
echo "ok: cdamux-hooks wire/unwire"
