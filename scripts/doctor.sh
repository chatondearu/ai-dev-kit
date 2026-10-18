#!/usr/bin/env bash
# doctor.sh — check host tools the kit expects; never installs packages.
# Exit 0 if at least one agent CLI is on PATH; 1 otherwise.
set -euo pipefail

# Resolve kit root without external dirname (NixOS PATH filters in tests).
_self="${BASH_SOURCE[0]:-$0}"
_dir="${_self%/*}"
[ "$_dir" = "$_self" ] && _dir="."
ROOT="$(cd "$_dir/.." && pwd)"
REC="$ROOT/docs/recommended.md"
FAILS=0
WARN=0

ok() { printf '  ok   %s\n' "$1"; }
miss() { printf '  miss %s\n' "$1"; FAILS=$((FAILS + 1)); }
warn() { printf '  warn %s\n' "$1"; WARN=$((WARN + 1)); }

have() { command -v "$1" >/dev/null 2>&1; }

printf 'ai-dev-kit doctor\n'
printf 'repo: %s\n\n' "$ROOT"

printf 'Core (git / gh)\n'
if have git; then
  _v="$(git --version 2>/dev/null || true)"
  ok "git (${_v:-present})"
else
  miss 'git'
fi
if have gh; then
  _v="$(gh --version 2>/dev/null | { read -r line || true; printf '%s' "$line"; })"
  ok "gh (${_v:-present})"
else
  warn 'gh (Kanban / CI watch needs it)'
fi

printf '\nAgent CLIs (need ≥1)\n'
AGENT_HIT=0
for c in agent cursor-agent claude opencode; do
  if have "$c"; then
    ok "$c ($(command -v "$c"))"
    AGENT_HIT=1
  else
    printf '  —    %s\n' "$c"
  fi
done
if [ "$AGENT_HIT" -eq 0 ]; then
  miss 'no agent CLI (tried agent, cursor-agent, claude, opencode)'
  printf '       Set ORCH_AGENT or install a CLI; see docs/recommended.md\n'
fi

printf '\nOptional\n'
if have bwrap; then ok 'bwrap (orch --sandbox)'; else printf '  —    bwrap (optional sandbox)\n'; fi
if have llama-swap; then ok "llama-swap ($(command -v llama-swap))"; else printf '  —    llama-swap (local models)\n'; fi

printf '\nKit install (skills linked?)\n'
SKILL_HIT=0
for d in \
  "${CURSOR_HOME:-$HOME/.cursor}/skills/cda-dev" \
  "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills/cda-dev" \
  "${XDG_CONFIG_HOME:-$HOME/.config}/opencode/skills/cda-dev"
do
  if [ -e "$d" ]; then
    ok "$d"
    SKILL_HIT=1
  fi
done
if [ "$SKILL_HIT" -eq 0 ]; then
  warn 'cda-dev skill not linked — run ./install.sh'
fi

printf '\nDocs\n'
if [ -f "$REC" ]; then
  ok "recommended tools: $REC"
else
  warn "missing $REC"
fi
printf '  tip  headless loop: bash skills/core/cda-dev/scripts/headless.sh --help\n'
printf '  tip  orchestrator:  bash skills/core/cda-dev/scripts/orch.sh --help\n'

printf '\n'
if [ "$FAILS" -gt 0 ]; then
  printf 'result: FAIL (%s missing, %s warn)\n' "$FAILS" "$WARN"
  exit 1
fi
printf 'result: OK (%s warn)\n' "$WARN"
exit 0
