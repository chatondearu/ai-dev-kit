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

printf '\nHost tools (jq / bash / flock)\n'
if have jq; then ok 'jq'; else warn 'jq (kiss hooks wiring + orch status need it)'; fi
# orch's lib/graph.sh uses mapfile (bash 4+); install.sh and seed-design.sh run on 3.2.
if [ "${BASH_VERSINFO[0]:-0}" -ge 4 ]; then
  ok "bash ${BASH_VERSION%%[(]*}"
else
  warn "bash ${BASH_VERSION%%[(]*} < 4: orch.sh needs bash 4+ (install.sh and seed-design.sh are fine)"
fi
if have flock; then ok 'flock'; else warn 'flock (optional: mkdir lock fallback is used)'; fi

printf '\nAgent CLIs (need ≥1)\n'
AGENT_HIT=0
for c in agent cursor-agent claude opencode codex; do
  if have "$c"; then
    ok "$c ($(command -v "$c"))"
    AGENT_HIT=1
  else
    printf '  —    %s\n' "$c"
  fi
done
if [ "$AGENT_HIT" -eq 0 ]; then
  miss 'no agent CLI (tried agent, cursor-agent, claude, opencode, codex)'
  printf '       Set ORCH_AGENT or install a CLI; see docs/recommended.md\n'
fi

# Cursor has no model aliases: a pinned id in the tier table rots silently (ADR-010).
# Warn only; offline / logged out is one info line. Skipped without a Cursor CLI.
CUR=''
for c in agent cursor-agent; do
  if have "$c"; then CUR="$c"; break; fi
done
if [ -n "$CUR" ]; then
  TIERS="${ORCH_TIERS_FILE:-$ROOT/skills/core/cda-dev/config/tiers.tsv}"
  printf '\nCursor models (%s)\n' "$TIERS"
  _models=''
  if have timeout; then
    _models="$(timeout 20 "$CUR" --list-models 2>/dev/null)" || _models=''
  else
    _models="$("$CUR" --list-models 2>/dev/null)" || _models=''
  fi
  if [ -z "$_models" ] || [ ! -r "$TIERS" ]; then
    printf '  —    %s --list-models unavailable (offline / logged out?) or tiers file unreadable; skipped\n' "$CUR"
  else
    # Drop ANSI colour codes, then match each id as a whole word of some line.
    _models="$(printf '%s\n' "$_models" | sed "s/$(printf '\033')\[[0-9;]*m//g")"
    while IFS="$(printf '\t')" read -r _cli _tier _flags || [ -n "${_cli:-}" ]; do
      [ "${_cli:-}" = cursor ] || continue
      _id=''
      _prev=''
      for _w in $_flags; do
        [ "$_prev" = --model ] && _id="$_w"
        case "$_w" in --model=*) _id="${_w#--model=}" ;; esac
        _prev="$_w"
      done
      [ -n "$_id" ] && [ "$_id" != auto ] || continue
      _hit=0
      while IFS= read -r _line; do
        for _w in $_line; do
          [ "$_w" = "$_id" ] && _hit=1
        done
      done <<EOF
$_models
EOF
      if [ "$_hit" -eq 1 ]; then
        ok "cursor/$_tier: $_id"
      else
        warn "cursor/$_tier: model '$_id' not in $CUR --list-models (update $TIERS)"
      fi
    done <"$TIERS"
  fi
fi

printf '\nOptional\n'
if have bwrap; then ok 'bwrap (orch --sandbox)'; else printf '  —    bwrap (optional sandbox)\n'; fi
if have llama-swap; then ok "llama-swap ($(command -v llama-swap))"; else printf '  —    llama-swap (local models)\n'; fi
if have graphify; then
  ok "graphify ($(command -v graphify))"
else
  printf '  —    graphify (cda.aitools.graphify / uv tool install graphifyy)\n'
fi
if [ "${CDA_GRAPHIFY:-}" = "1" ]; then
  ok 'CDA_GRAPHIFY=1'
elif [ -n "${CDA_GRAPHIFY:-}" ]; then
  warn "CDA_GRAPHIFY=${CDA_GRAPHIFY} (expected 1 when graphify is enabled)"
else
  printf '  —    CDA_GRAPHIFY (set by cda.aitools.graphify.enable)\n'
fi

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
printf '  tip  AFK Ready poller: bash skills/core/cda-dev/scripts/orch-auto.sh --help\n'
printf '  tip  systemd templates: skills/core/cda-dev/templates/systemd/ (opt-in)\n'
printf '  tip  orchestrator:  bash skills/core/cda-dev/scripts/orch.sh --help\n'

printf '\n'
if [ "$FAILS" -gt 0 ]; then
  printf 'result: FAIL (%s missing, %s warn)\n' "$FAILS" "$WARN"
  exit 1
fi
printf 'result: OK (%s warn)\n' "$WARN"
exit 0
