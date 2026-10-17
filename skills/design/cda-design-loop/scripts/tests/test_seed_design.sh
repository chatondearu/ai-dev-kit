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
echo "test_seed_design: ok"
