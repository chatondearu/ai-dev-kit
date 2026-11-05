# Shared graphify activation oneshot script for darwin + NixOS services.
{
  pkgs,
  lib,
  graphifyPkg,
  a,
  pathTail,
}:
let
  ensureUv =
    a.graphify.installMethod == "uv"
    || a.graphify.installMethod == "auto";
  useNixPkg =
    graphifyPkg != null
    && (
      a.graphify.installMethod == "nix"
      || a.graphify.installMethod == "auto"
    );
  activateArgs = lib.optionalString ensureUv " --ensure-uv";
  pathPrefix =
    lib.optionalString useNixPkg "${graphifyPkg}/bin:"
    + lib.optionalString ensureUv "${pkgs.uv}/bin:";
in
  pkgs.writeShellScript "cda-graphify-activate" ''
    set -euo pipefail
    export PATH="${pathPrefix}$HOME/.local/bin:${pathTail}"
    exec ${pkgs.bash}/bin/bash ${../scripts/graphify-activate.sh}${activateArgs}
  ''
