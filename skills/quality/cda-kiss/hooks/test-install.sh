#!/usr/bin/env bash
# install.sh — hook-wiring test: drives the real CLI against temp config dirs only.
set -euo pipefail
cd "$(dirname "$0")/../../../.."   # repo root

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" CLAUDE_CONFIG_DIR="$tmp/claude" CURSOR_HOME="$tmp/cursor" XDG_CONFIG_HOME="$tmp/xdg"
mkdir -p "$HOME" "$CLAUDE_CONFIG_DIR" "$CURSOR_HOME"
S="$CLAUDE_CONFIG_DIR/settings.json" C="$CURSOR_HOME/hooks.json" K="$PWD/skills/quality/cda-kiss/hooks"

fail() { echo "FAIL: $*" >&2; exit 1; }
SH=bash; [ -x /bin/bash ] && SH=/bin/bash  # stock bash (3.2 on macOS) must work
inst() { "$SH" ./install.sh --claude --cursor --no-opencode "$@" >"$tmp/out" 2>&1 || { cat "$tmp/out" >&2; fail "install.sh $*"; }; }
count() { jq --arg k "$K" "[$2 | select(contains(\$k))] | length" "$1"; }
claude_cmds='.hooks[][].hooks[]? | (.command // empty)'
cursor_cmds='.hooks[][].command'

# pre-existing user config: a key + a foreign hook in each file
# (the Stop hook is type "prompt": it has no .command)
echo '{"model":"opus","hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo mine"}]}],"Stop":[{"hooks":[{"type":"prompt","prompt":"keep"}]}]}}' > "$S"
echo '{"version":1,"hooks":{"sessionStart":[{"command":"echo mine"}]}}' > "$C"

inst; inst   # second run must be a no-op
[ "$(jq -r .model "$S")" = opus ] || fail "model key lost"
jq -e '[.hooks.SessionStart[].hooks[].command] | index("echo mine")' "$S" >/dev/null || fail "foreign claude hook lost"
jq -e '[.hooks.sessionStart[].command] | index("echo mine")' "$C" >/dev/null || fail "foreign cursor hook lost"
jq -e '.hooks.Stop[0].hooks[0].prompt == "keep"' "$S" >/dev/null || fail "prompt-type hook lost"
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
jq -e '.hooks.Stop | length == 1' "$S" >/dev/null || fail "prompt-type hook removed by uninstall"
[ ! -e "$CLAUDE_CONFIG_DIR/CLAUDE.md" ] && [ ! -L "$CLAUDE_CONFIG_DIR/CLAUDE.md" ] || fail "CLAUDE.md link left after uninstall"

# cdamux sub-agent ledger hooks: opt-in only, idempotent, removed by --uninstall
D="$PWD/cdamux/hooks"
dcount() { jq --arg d "$D" "[$claude_cmds | select(contains(\$d))] | length" "$S"; }
inst; [ "$(dcount)" = 0 ] || fail "cdamux hooks wired without --cdamux-hooks"
inst --cdamux-hooks; inst --cdamux-hooks
[ "$(dcount)" = 3 ] || fail "cdamux hooks not exactly once"
jq -e --arg d "$D" '[.hooks.SubagentStop[].hooks[].command | select(contains($d))] | length == 1' "$S" >/dev/null || fail "no cdamux SubagentStop hook"
[ "$(count "$S" "$claude_cmds")" = 3 ] || fail "kiss hooks changed by --cdamux-hooks"
# SessionStart: foreign + kiss + cdamux side by side; a plain re-install (kiss rewire) keeps cdamux
ss_ok() { jq -e --arg d "$D" --arg k "$K" '[.hooks.SessionStart[].hooks[].command] as $c
  | ($c | index("echo mine")) and ([$c[] | select(contains($k))] | length == 1) and ([$c[] | select(contains($d))] | length == 1)' "$S" >/dev/null; }
ss_ok || fail "claude SessionStart: kiss and cdamux not both present once"
inst; ss_ok || fail "plain install dropped the cdamux SessionStart hook"
ccount() { jq --arg d "$D" "[$cursor_cmds | select(contains(\$d))] | length" "$C"; }
[ "$(ccount)" = 3 ] || fail "cursor cdamux hooks not exactly once"
jq -e --arg d "$D" '[.hooks.subagentStop[].command | select(contains($d))] | length == 1' "$C" >/dev/null || fail "no cdamux subagentStop hook"
jq -e --arg d "$D" --arg k "$K" '[.hooks.sessionStart[].command] as $c
  | ([$c[] | select(contains($k))] | length == 1) and ([$c[] | select(contains($d))] | length == 1)' "$C" >/dev/null || fail "cursor sessionStart: kiss and cdamux not both present once"
jq -e '.hooks.sessionStart | map(.command) | index("echo mine")' "$C" >/dev/null || fail "foreign cursor hook lost by --cdamux-hooks"
OCL="$XDG_CONFIG_HOME/opencode/plugins/cdamux-subagent.js"
"$SH" ./install.sh --opencode --no-claude --no-cursor >"$tmp/out" 2>&1 || fail "opencode install"
[ ! -e "$OCL" ] || fail "opencode plugin linked without --cdamux-hooks"
"$SH" ./install.sh --opencode --no-claude --no-cursor --cdamux-hooks >"$tmp/out" 2>&1 || fail "opencode install --cdamux-hooks"
[ "$(readlink "$OCL")" = "$D/opencode-subagent.js" ] || fail "opencode plugin not linked"
inst --uninstall
"$SH" ./install.sh --opencode --no-claude --no-cursor --uninstall >"$tmp/out" 2>&1 || fail "opencode uninstall"
[ ! -L "$OCL" ] || fail "opencode plugin link left after uninstall"
[ "$(dcount)" = 0 ] && [ "$(ccount)" = 0 ] || fail "cdamux hooks left after uninstall"
jq -e '.hooks.SubagentStop == null' "$S" >/dev/null || fail "empty SubagentStop event left"
jq -e '[.hooks.SessionStart[].hooks[].command] == ["echo mine"]' "$S" >/dev/null || fail "claude SessionStart after uninstall wrong"
jq -e '.hooks.subagentStop == null and (.hooks.sessionStart | length == 1)' "$C" >/dev/null || fail "cursor cdamux uninstall wrong"

# --cursor-home without a value: clear error, not an unbound-variable crash
! "$SH" ./install.sh --cursor-home >"$tmp/out" 2>&1 || fail "--cursor-home without DIR accepted"
grep -q 'needs a DIR' "$tmp/out" || fail "no clear --cursor-home error"

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
