#!/usr/bin/env bash
# macOS: skzv/tap/ccmux + sync configs from ai-dev-kit aitools/templates/ccmux.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script is for macOS only." >&2
  exit 1
fi

AITOOLS_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEMPLATES="$AITOOLS_ROOT/templates/ccmux"

command -v brew >/dev/null || {
  echo "Homebrew is required." >&2
  exit 1
}

echo "==> Removing Homebrew core ccmux (epilande) if present"
if brew list --formula ccmux &>/dev/null; then
  brew uninstall ccmux
fi

echo "==> Installing skzv/tap/ccmux"
brew tap skzv/tap 2>/dev/null || true
if brew trust --help &>/dev/null; then
  brew trust --tap skzv/tap 2>/dev/null || true
fi
brew install skzv/tap/ccmux

echo "==> Syncing config from $TEMPLATES"
mkdir -p "$HOME/.config/ccmux"
cp "$TEMPLATES/ccmux.json" "$HOME/.config/ccmux/ccmux.json"
cp "$TEMPLATES/config.toml" "$HOME/.config/ccmux/config.toml"

if grep -q '@PROJECTS_ROOT@' "$HOME/.config/ccmux/config.toml" 2>/dev/null; then
  DEV_ROOT="${HOME}/dev"
  sed -i '' "s|@PROJECTS_ROOT@|${DEV_ROOT}|g" "$HOME/.config/ccmux/config.toml"
  echo "Set projects.root to $DEV_ROOT — edit ~/.config/ccmux/config.toml if needed."
fi

echo "==> ccmux setup (hooks)"
ccmux setup

echo ""
echo "Done. Version: $(ccmux --version)"
echo "Moshi (Cursor): bash aitools/scripts/moshi-hook-bootstrap.sh --target cursor"
echo "nix-darwin: use darwinModules.cdaAitools with deploy.enable instead of manual copies."
