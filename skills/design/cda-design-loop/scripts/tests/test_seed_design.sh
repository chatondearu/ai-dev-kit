#!/usr/bin/env bash
set -euo pipefail
# tests → scripts → cda-design-loop → design → skills → repo (5× ..)
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SEED="$ROOT/skills/design/cda-design-loop/scripts/seed-design.sh"
SH=bash; [ -x /bin/bash ] && SH=/bin/bash  # stock bash (3.2 on macOS) must work
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/proj"
"$SH" "$SEED" --target "$TMP/proj"
test -f "$TMP/proj/DESIGN.md"
test -f "$TMP/proj/design/tokens.json"
test -x "$TMP/proj/scripts/cda-design-shot.sh"
grep -q 'Taste stub' "$TMP/proj/DESIGN.md"
# idempotent: second run without --force must not fail
"$SH" "$SEED" --target "$TMP/proj"
# --no-shot / --no-tokens
TMP2="$(mktemp -d)"
mkdir -p "$TMP2/proj"
"$SH" "$SEED" --target "$TMP2/proj" --no-shot --no-tokens
test ! -e "$TMP2/proj/scripts/cda-design-shot.sh"
test ! -e "$TMP2/proj/design/tokens.json"
# --brand linear
TMP3="$(mktemp -d)"
mkdir -p "$TMP3/proj"
"$SH" "$SEED" --target "$TMP3/proj" --brand linear --no-tokens --no-shot
grep -qi 'linear' "$TMP3/proj/DESIGN.md"
# brand name is case-insensitive; unknown brand is rejected
"$SH" "$SEED" --target "$TMP3/proj" --brand LINEAR --force --no-tokens --no-shot
! "$SH" "$SEED" --target "$TMP3/proj" --brand nope 2>/dev/null
# docs/DESIGN.md only (custom brief, no brand): must not add root taste stub
TMP4="$(mktemp -d)"
mkdir -p "$TMP4/proj/docs"
CUSTOM='Our product design principles — no kit brand here.'
printf '%s\n' "$CUSTOM" >"$TMP4/proj/docs/DESIGN.md"
"$SH" "$SEED" --target "$TMP4/proj" --no-tokens --no-shot
test ! -e "$TMP4/proj/DESIGN.md"
grep -q "$CUSTOM" "$TMP4/proj/docs/DESIGN.md"
# linear-gradient alone must not trigger Linear brand pack
TMP5="$(mktemp -d)"
mkdir -p "$TMP5/proj"
printf '%s\n' 'background: linear-gradient(180deg, #000, #111);' >"$TMP5/proj/DESIGN.md"
"$SH" "$SEED" --target "$TMP5/proj" --no-tokens --no-shot
grep -q 'linear-gradient' "$TMP5/proj/DESIGN.md"
grep -qi 'linear-design-analysis' "$TMP5/proj/DESIGN.md" && exit 1 || true
# --force refreshes taste stub over a modified stub (no brand in brief)
TMP6="$(mktemp -d)"
mkdir -p "$TMP6/proj"
"$SH" "$SEED" --target "$TMP6/proj" --no-tokens --no-shot
printf '%s\n' 'MODIFIED BRIEF' >>"$TMP6/proj/DESIGN.md"
"$SH" "$SEED" --target "$TMP6/proj" --force --no-tokens --no-shot
grep -q 'Taste stub' "$TMP6/proj/DESIGN.md"
grep -q 'MODIFIED BRIEF' "$TMP6/proj/DESIGN.md" && exit 1
echo "test_seed_design: ok"
