#!/usr/bin/env bash
# seed-design.sh — drop DESIGN.md (+ optional tokens/shot) into a target repo.
set -euo pipefail
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRANDS_DIR="$SKILL_DIR/../cda-design-md/brands"

declare -A BRAND_DIR=(
  [linear]=linear.app
  [notion]=notion
  [cal]=cal
  [raycast]=raycast
  [cursor]=cursor
  [resend]=resend
)
KNOWN_BRANDS=(linear notion cal raycast cursor resend)

TARGET=""
FORCE=0
NO_TOKENS=0
NO_SHOT=0
BRAND=""

usage() {
  echo "Usage: $(basename "$0") --target DIR [--force] [--no-tokens] [--no-shot] [--brand NAME]" >&2
  exit 1
}

brand_dir_for() {
  local key="${1,,}"
  [[ -n "${BRAND_DIR[$key]:-}" ]] || return 1
  echo "${BRAND_DIR[$key]}"
}

detect_brand_in_file() {
  local f="$1"
  [[ -f "$f" ]] || return 1
  local b
  for b in "${KNOWN_BRANDS[@]}"; do
    if grep -qi "$b" "$f"; then
      echo "$b"
      return 0
    fi
  done
  return 1
}

existing_brief() {
  local t="$1"
  if [[ -f "$t/DESIGN.md" ]]; then
    echo "$t/DESIGN.md"
    return 0
  fi
  if [[ -f "$t/docs/DESIGN.md" ]]; then
    echo "$t/docs/DESIGN.md"
    return 0
  fi
  return 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGET="${2:-}"; shift 2 ;;
    --force) FORCE=1; shift ;;
    --no-tokens) NO_TOKENS=1; shift ;;
    --no-shot) NO_SHOT=1; shift ;;
    --brand) BRAND="${2:-}"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

[[ -n "$TARGET" ]] || usage
TARGET="$(cd "$TARGET" 2>/dev/null && pwd)" || {
  echo "seed-design: target is not a directory: $TARGET" >&2
  exit 1
}

written=()
skipped=()

copy_brand_brief() {
  local name="$1"
  local dir
  dir="$(brand_dir_for "$name")" || return 1
  local src="$BRANDS_DIR/$dir/DESIGN.md"
  [[ -f "$src" ]] || return 1
  cp "$src" "$TARGET/DESIGN.md"
  written+=("DESIGN.md (brand: ${name,,})")
}

if [[ -n "$BRAND" ]]; then
  bkey="${BRAND,,}"
  if ! brand_dir_for "$bkey" >/dev/null; then
    echo "seed-design: invalid brand: $BRAND" >&2
    exit 1
  fi
  src="$BRANDS_DIR/$(brand_dir_for "$bkey")/DESIGN.md"
  if [[ ! -f "$src" ]]; then
    echo "seed-design: brand pack missing: $BRAND" >&2
    exit 1
  fi
  if [[ -f "$TARGET/DESIGN.md" && "$FORCE" -eq 0 ]]; then
    skipped+=("DESIGN.md")
  else
    copy_brand_brief "$bkey"
  fi
else
  brief_path=""
  detected=""
  if brief_path="$(existing_brief "$TARGET" 2>/dev/null)"; then
    detected="$(detect_brand_in_file "$brief_path" 2>/dev/null || true)"
  fi
  if [[ -n "$detected" ]]; then
    if [[ "$FORCE" -eq 1 ]]; then
      copy_brand_brief "$detected"
    else
      skipped+=("DESIGN.md")
    fi
  elif [[ -f "$TARGET/DESIGN.md" && "$FORCE" -eq 0 ]]; then
    skipped+=("DESIGN.md")
  else
    cp "$SKILL_DIR/templates/DESIGN.stub.md" "$TARGET/DESIGN.md"
    written+=("DESIGN.md (taste stub)")
  fi
fi

if [[ "$NO_TOKENS" -eq 0 ]]; then
  tok="$TARGET/design/tokens.json"
  if [[ -f "$tok" && "$FORCE" -eq 0 ]]; then
    skipped+=("design/tokens.json")
  else
    mkdir -p "$TARGET/design"
    cp "$SKILL_DIR/templates/tokens.stub.json" "$tok"
    written+=("design/tokens.json")
  fi
fi

if [[ "$NO_SHOT" -eq 0 ]]; then
  shot="$TARGET/scripts/cda-design-shot.sh"
  if [[ -f "$shot" && "$FORCE" -eq 0 ]]; then
    skipped+=("scripts/cda-design-shot.sh")
  else
    mkdir -p "$TARGET/scripts"
    cp "$SKILL_DIR/templates/cda-design-shot.sh" "$shot"
    chmod +x "$shot"
    written+=("scripts/cda-design-shot.sh")
  fi
fi

summary=""
if ((${#written[@]})); then
  summary="wrote: ${written[*]}"
fi
if ((${#skipped[@]})); then
  [[ -n "$summary" ]] && summary="$summary; "
  summary="${summary}skipped: ${skipped[*]}"
fi
[[ -n "$summary" ]] || summary="nothing to do"
echo "seed-design: $summary"
