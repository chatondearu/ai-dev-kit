#!/usr/bin/env bash
# Idempotent graphify skill registration for cda.aitools activation.
# Honours CLAUDE_CONFIG_DIR. Never installs packages itself (uv/nix do that).
# Usage: graphify-activate.sh [--ensure-uv]
#
# Safety: runs in a throwaway cwd so project-scoped writers cannot dirty a kit
# checkout. When $CLAUDE_CONFIG_DIR/CLAUDE.md (or ~/.claude/CLAUDE.md) is a
# symlink into a managed tree (e.g. kit claude/CLAUDE.md), registration is
# shielded so the symlink target is never written.
set -euo pipefail

ENSURE_UV=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ensure-uv) ENSURE_UV=1; shift ;;
    -h | --help)
      printf 'Usage: %s [--ensure-uv]\n' "$(basename "$0")"
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

claude_cfg="${CLAUDE_CONFIG_DIR:-${HOME}/.claude}"
claude_md="${claude_cfg}/CLAUDE.md"

# Throwaway cwd: graphify project writers target cwd.
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/cda-graphify-activate.XXXXXX")"
SHIELDED=0
CLAUDE_MD_LINK_TARGET=""
cleanup() {
  if [[ "$SHIELDED" -eq 1 ]]; then
    rm -f "$claude_md"
    ln -s "$CLAUDE_MD_LINK_TARGET" "$claude_md"
  fi
  rm -rf "$WORKDIR"
}
trap cleanup EXIT
cd "$WORKDIR"

# Shield: install writes CLAUDE.md through symlinks into managed kit trees.
if [[ -L "$claude_md" ]]; then
  CLAUDE_MD_LINK_TARGET="$(readlink "$claude_md")"
  tmp_real="$(mktemp "${claude_md}.XXXXXX")"
  cp -p "$claude_md" "$tmp_real"
  rm "$claude_md"
  mv "$tmp_real" "$claude_md"
  SHIELDED=1
  graphify install
  rm -f "$claude_md"
  ln -s "$CLAUDE_MD_LINK_TARGET" "$claude_md"
  SHIELDED=0
  echo "graphify-activate: restored symlinked CLAUDE.md (skill installed; skip kit registration)" >&2
else
  graphify install
fi
