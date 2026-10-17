#!/usr/bin/env bash
set -euo pipefail
# tests → scripts → cda-design-loop → design → skills → repo (5× ..)
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SEED="$ROOT/skills/design/cda-design-loop/scripts/seed-design.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/proj"
bash "$SEED" --target "$TMP/proj"
test -f "$TMP/proj/DESIGN.md"
test -f "$TMP/proj/design/tokens.json"
test -x "$TMP/proj/scripts/cda-design-shot.sh"
grep -q 'Taste stub' "$TMP/proj/DESIGN.md"
# idempotent: second run without --force must not fail
bash "$SEED" --target "$TMP/proj"
# --no-shot / --no-tokens
TMP2="$(mktemp -d)"
mkdir -p "$TMP2/proj"
bash "$SEED" --target "$TMP2/proj" --no-shot --no-tokens
test ! -e "$TMP2/proj/scripts/cda-design-shot.sh"
test ! -e "$TMP2/proj/design/tokens.json"
# --brand linear
TMP3="$(mktemp -d)"
mkdir -p "$TMP3/proj"
bash "$SEED" --target "$TMP3/proj" --brand linear --no-tokens --no-shot
grep -qi 'linear' "$TMP3/proj/DESIGN.md"
# docs/DESIGN.md only (custom brief, no brand): must not add root taste stub
TMP4="$(mktemp -d)"
mkdir -p "$TMP4/proj/docs"
CUSTOM='Our product design principles — no kit brand here.'
printf '%s\n' "$CUSTOM" >"$TMP4/proj/docs/DESIGN.md"
bash "$SEED" --target "$TMP4/proj" --no-tokens --no-shot
test ! -e "$TMP4/proj/DESIGN.md"
grep -q "$CUSTOM" "$TMP4/proj/docs/DESIGN.md"
# linear-gradient alone must not trigger Linear brand pack
TMP5="$(mktemp -d)"
mkdir -p "$TMP5/proj"
printf '%s\n' 'background: linear-gradient(180deg, #000, #111);' >"$TMP5/proj/DESIGN.md"
bash "$SEED" --target "$TMP5/proj" --no-tokens --no-shot
grep -q 'linear-gradient' "$TMP5/proj/DESIGN.md"
grep -qi 'linear-design-analysis' "$TMP5/proj/DESIGN.md" && exit 1 || true
# --force refreshes taste stub over a modified stub (no brand in brief)
TMP6="$(mktemp -d)"
mkdir -p "$TMP6/proj"
bash "$SEED" --target "$TMP6/proj" --no-tokens --no-shot
printf '%s\n' 'MODIFIED BRIEF' >>"$TMP6/proj/DESIGN.md"
bash "$SEED" --target "$TMP6/proj" --force --no-tokens --no-shot
grep -q 'Taste stub' "$TMP6/proj/DESIGN.md"
grep -q 'MODIFIED BRIEF' "$TMP6/proj/DESIGN.md" && exit 1
echo "test_seed_design: ok"
