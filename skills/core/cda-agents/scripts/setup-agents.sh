#!/usr/bin/env bash
# setup-agents.sh — create or update AGENTS.md for a target repository.
#
# Usage:
#   ./setup-agents.sh [--dry-run] [--force] [--type TYPE] [--init-foam] [TARGET_DIR]
#
# TYPE: web | api | infra | fullstack | minimal | auto (default: auto)
set -euo pipefail

SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMPLATES="$SKILL_DIR/templates"
FOAM_INIT="$SKILL_DIR/../cda-foam/scripts/init.sh"
DRY=0
FORCE=0
INIT_FOAM=0
TYPE="auto"
TARGET=""

MARKER_BEGIN='<!-- agents-setup:managed -->'
MARKER_END='<!-- /agents-setup:managed -->'

log() { printf '%s\n' "$*"; }
run() { if [ "$DRY" = 1 ]; then printf 'DRY  %s\n' "$*"; else eval "$*"; fi; }
die() { printf 'setup-agents.sh: %s\n' "$*" >&2; exit 1; }

usage() {
  sed -n '2,12p' "$0"
  exit 0
}

resolve_target() {
  if [ -n "$TARGET" ]; then
    cd "$TARGET" || die "cannot cd to $TARGET"
  fi
  if git rev-parse --show-toplevel >/dev/null 2>&1; then
    cd "$(git rev-parse --show-toplevel)"
  fi
  pwd
}

detect_type() {
  local has_vue=0 has_api=0 has_nix=0
  if [ -f package.json ] && grep -qE '"nuxt"|"vue"' package.json 2>/dev/null; then has_vue=1; fi
  [ -f nuxt.config.ts ] || [ -f nuxt.config.js ] && has_vue=1
  [ -f pyproject.toml ] || [ -f requirements.txt ] || [ -f go.mod ] && has_api=1
  [ -f openapi.yaml ] || [ -f openapi.json ] && has_api=1
  [ -f src/main.py ] || [ -f src/main.rs ] && has_api=1
  [ -f flake.nix ] && has_nix=1

  if [ "$has_vue" = 1 ] && { [ "$has_api" = 1 ] || [ "$has_nix" = 1 ]; }; then
    printf 'fullstack'
  elif [ "$has_vue" = 1 ]; then printf 'web'
  elif [ "$has_api" = 1 ]; then printf 'api'
  elif [ "$has_nix" = 1 ]; then printf 'infra'
  else printf 'minimal'
  fi
}

assemble_managed_block() {
  local project_type="$1" tmp overlay
  tmp="$(mktemp)"
  cp "$TEMPLATES/AGENTS.base.md" "$tmp"

  overlay="$TEMPLATES/overlays/${project_type}.md"
  if [ -f "$overlay" ]; then
    awk -v overlay="$overlay" '
      /<!-- TYPE_OVERLAY -->/ {
        while ((getline line < overlay) > 0) print line
        close(overlay)
        next
      }
      { print }
    ' "$tmp" > "${tmp}.out"
    mv "${tmp}.out" "$tmp"
  else
    sed '/<!-- TYPE_OVERLAY -->/d' "$tmp" > "${tmp}.out"
    mv "${tmp}.out" "$tmp"
  fi

  awk -v begin="$MARKER_BEGIN" -v end="$MARKER_END" '
    $0 == begin { emit=1 }
    emit { print }
    $0 == end { emit=0 }
  ' "$tmp"
  rm -f "$tmp"
}

write_full_agents() {
  local project_type="$1" dest="$REPO_ROOT/AGENTS.md" block tmp_notes
  block="$(assemble_managed_block "$project_type")"
  tmp_notes="$(mktemp)"

  if [ -f "$dest" ] && [ "$FORCE" != 1 ]; then
    die "AGENTS.md exists — use --force or let patch path handle it"
  fi

  if [ -f "$dest" ]; then
    awk '
      /^## Project-specific notes/ { found=1 }
      found { print }
    ' "$dest" > "$tmp_notes" || true
  fi

  if [ ! -s "$tmp_notes" ]; then
    cat > "$tmp_notes" <<'EOF'
## Project-specific notes

_Add stack versions, deployment URLs, gotchas, and links below._
EOF
  fi

  if [ "$DRY" = 1 ]; then
    log "DRY  would write $dest (type=$project_type)"
    rm -f "$tmp_notes"
    return
  fi

  {
    printf '# Agent guide\n\n'
    printf '%s\n' "$block"
    printf '\n'
    cat "$tmp_notes"
  } > "$dest"
  rm -f "$tmp_notes"
  log "write AGENTS.md (type=$project_type)"
}

patch_agents() {
  local project_type="$1" dest="$REPO_ROOT/AGENTS.md" block tmp out
  [ -f "$dest" ] || { write_full_agents "$project_type"; return; }

  block="$(assemble_managed_block "$project_type")"
  tmp="$(mktemp)"
  out="$(mktemp)"

  if ! grep -qF "$MARKER_BEGIN" "$dest"; then
    if [ "$FORCE" != 1 ]; then
      log "skip AGENTS.md (no managed markers; use --force to prepend)"
      rm -f "$tmp" "$out"
      return
    fi
    if [ "$DRY" = 1 ]; then
      log "DRY  prepend managed block"
      rm -f "$tmp" "$out"
      return
    fi
    {
      printf '# Agent guide\n\n'
      printf '%s\n\n' "$block"
      cat "$dest"
    } > "$out"
    mv "$out" "$dest"
    log "update AGENTS.md (prepended managed block)"
    rm -f "$tmp" "$out"
    return
  fi

  if [ "$DRY" = 1 ]; then
    log "DRY  replace managed block (type=$project_type)"
    rm -f "$tmp" "$out"
    return
  fi

  awk -v begin="$MARKER_BEGIN" -v end="$MARKER_END" '
    BEGIN { inblock=0; done=0 }
    $0 == begin {
      while ((getline line < BLOCK) > 0) print line
      close("BLOCK")
      inblock=1
      done=1
      next
    }
    inblock {
      if ($0 == end) inblock=0
      next
    }
    { print }
    END { if (!done) { print "markers missing" > "/dev/stderr"; exit 1 } }
  ' BLOCK=<(printf '%s\n' "$block") "$dest" > "$out"

  mv "$out" "$dest"
  rm -f "$tmp"
  log "update AGENTS.md (managed block, type=$project_type)"
}

main() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --dry-run) DRY=1;;
      --force) FORCE=1;;
      --init-foam) INIT_FOAM=1;;
      --type) TYPE="${2:?--type requires value}"; shift;;
      -h|--help) usage;;
      -*) die "unknown option: $1";;
      *) TARGET="$1";;
    esac
    shift
  done

  [ -f "$TEMPLATES/AGENTS.base.md" ] || die "missing AGENTS.base.md"

  REPO_ROOT="$(resolve_target)"
  log "repo : $REPO_ROOT"

  if [ "$TYPE" = auto ]; then
    TYPE="$(detect_type)"
    log "type : $TYPE (auto-detected)"
  else
    log "type : $TYPE"
  fi

  if [ "$INIT_FOAM" = 1 ]; then
    if [ -d "$REPO_ROOT/foam" ] && [ "$FORCE" != 1 ]; then
      log "ok   foam/ exists"
    elif [ -f "$FOAM_INIT" ]; then
      log "# foam scaffold"
      foam_args=()
      [ "$DRY" = 1 ] && foam_args+=(--dry-run)
      [ "$FORCE" = 1 ] && foam_args+=(--force)
      bash "$FOAM_INIT" "${foam_args[@]}" "$REPO_ROOT"
    else
      log "warn: missing $FOAM_INIT"
    fi
  fi

  if [ ! -f "$REPO_ROOT/AGENTS.md" ]; then
    write_full_agents "$TYPE"
  else
    patch_agents "$TYPE"
  fi

  log ""
  log "done."
}

main "$@"
