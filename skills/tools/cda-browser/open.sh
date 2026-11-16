#!/usr/bin/env bash
# Open a URL or filesystem path in Zen (neo-nix). Fallback: xdg-open / open.
set -euo pipefail
target="${1:?usage: open.sh <url-or-path>}"

if [[ "$target" != http://* && "$target" != https://* && "$target" != file://* ]]; then
  target="file://$(readlink -f "$target")"
fi

if command -v niri >/dev/null 2>&1 && command -v zen >/dev/null 2>&1; then
  niri msg action spawn -- zen --new-tab "$target"
  exit 0
fi
if command -v zen >/dev/null 2>&1; then
  zen --new-tab "$target" >/dev/null 2>&1 &
  exit 0
fi
if command -v xdg-open >/dev/null 2>&1; then
  xdg-open "$target"
  exit 0
fi
if command -v open >/dev/null 2>&1; then
  open "$target"
  exit 0
fi
printf 'open a browser at: %s\n' "$target" >&2
exit 1
