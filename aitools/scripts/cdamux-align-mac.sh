#!/usr/bin/env bash
# macOS: build/install cdamux from ai-dev-kit and sync config template.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CDAMUX_DIR="$REPO_ROOT/cdamux"

echo "==> Building cdamux (release)"
cd "$CDAMUX_DIR"
cargo build --release -p cdamux-cli -p cdamuxd

mkdir -p "$HOME/.local/bin"
install -m755 "$CDAMUX_DIR/target/release/cdamux" "$HOME/.local/bin/cdamux"
install -m755 "$CDAMUX_DIR/target/release/cdamuxd" "$HOME/.local/bin/cdamuxd"

mkdir -p "$HOME/.config/cdamux"
if [[ ! -f "$HOME/.config/cdamux/config.toml" ]]; then
  cp "$REPO_ROOT/aitools/templates/cdamux/config.toml" "$HOME/.config/cdamux/config.toml"
  echo "Edit projects.roots in $HOME/.config/cdamux/config.toml for this host."
fi

echo "Done. cdamux: $($HOME/.local/bin/cdamux --version 2>/dev/null || echo built)"
echo "Start daemon: cdamuxd &  (or use cda.aitools.cdamuxd.enable on Nix)"
