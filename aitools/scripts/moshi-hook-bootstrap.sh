#!/usr/bin/env bash
# One-time Moshi pairing + hook install (when Nix has not paired yet).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: moshi-hook-bootstrap.sh [--token TOKEN] [--target cursor] [--pair-only] [--install-only]

Pair this host with the Moshi iOS app and install agent hooks (default: cursor only).
Requires moshi-hook on PATH (NixOS: cda.aitools.moshiHook.enable).

  --token TOKEN     Pairing token (else MOSHI_PAIRING_TOKEN or prompt)
  --target LIST     Comma-separated agents for moshi-hook install (default: cursor)
  --pair-only       Skip moshi-hook install
  --install-only    Skip pair (host already paired)
EOF
}

TARGETS="cursor"
PAIR_ONLY=0
INSTALL_ONLY=0
TOKEN="${MOSHI_PAIRING_TOKEN:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --token)
      TOKEN="$2"
      shift 2
      ;;
    --target)
      TARGETS="$2"
      shift 2
      ;;
    --pair-only)
      PAIR_ONLY=1
      shift
      ;;
    --install-only)
      INSTALL_ONLY=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

command -v moshi-hook >/dev/null || {
  echo "moshi-hook not on PATH" >&2
  exit 1
}

export MOSHI_HOOK_SKIP_FIRST_RUN=1

if [[ "$INSTALL_ONLY" != "1" ]]; then
  if moshi-hook status --json 2>/dev/null | jq -e '.paired == true' >/dev/null; then
    echo "already paired"
  else
    if [[ -z "$TOKEN" ]]; then
      read -r -p "Moshi pairing token (app → Settings → Integrations): " TOKEN
    fi
    moshi-hook pair --token "$TOKEN" --store file
  fi
fi

if [[ "$PAIR_ONLY" != "1" ]]; then
  moshi-hook install --target "$TARGETS"
  systemctl --user restart moshi-hook.service 2>/dev/null || moshi-hook serve &
fi

moshi-hook doctor || true
