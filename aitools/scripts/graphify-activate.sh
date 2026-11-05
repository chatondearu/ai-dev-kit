#!/usr/bin/env bash
# Idempotent graphify skill registration for cda.aitools activation.
# Honours CLAUDE_CONFIG_DIR. Never installs packages itself (uv/nix do that).
# Usage: graphify-activate.sh [--hooks] [--ensure-uv]
set -euo pipefail

HOOKS=0
ENSURE_UV=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --hooks) HOOKS=1; shift ;;
    --ensure-uv) ENSURE_UV=1; shift ;;
    -h | --help)
      printf 'Usage: %s [--hooks] [--ensure-uv]\n' "$(basename "$0")"
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

# uv tool shims land here; keep ahead of system PATH.
export PATH="${HOME}/.local/bin:${PATH}"

ensure_graphify() {
  if command -v graphify >/dev/null 2>&1; then
    return 0
  fi
  if [[ "$ENSURE_UV" -ne 1 ]]; then
    echo "graphify-activate: graphify not on PATH (set installMethod=nix|uv|auto)" >&2
    return 1
  fi
  if ! command -v uv >/dev/null 2>&1; then
    echo "graphify-activate: uv not on PATH; cannot install graphifyy" >&2
    return 1
  fi
  uv tool install graphifyy
  command -v graphify >/dev/null 2>&1
}

ensure_graphify

# Register /graphify skill for detected assistants (idempotent upstream).
graphify install

if [[ "$HOOKS" -eq 1 ]]; then
  # Claude PreToolUse hooks; CLAUDE_CONFIG_DIR redirects writes away from live config.
  graphify claude install
fi
