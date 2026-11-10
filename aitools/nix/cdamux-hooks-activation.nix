# Shared cdamux hooks activation oneshot for darwin + NixOS services.
{
  pkgs,
  lib,
  cdamuxPkg,
  a,
}:
pkgs.writeShellScript "cda-cdamux-hooks-activate" ''
  set -euo pipefail
  ${lib.optionalString (a.cdamux.hooks.claudeConfigDir != null) "export CLAUDE_CONFIG_DIR=${lib.escapeShellArg a.cdamux.hooks.claudeConfigDir}"}
  ${cdamuxPkg}/bin/cdamux-hooks wire
  echo "cdamux hooks wired; running agents keep the old ones until: cdamux restart --all"
''
