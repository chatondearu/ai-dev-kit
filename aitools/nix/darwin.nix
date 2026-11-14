# nix-darwin entry: cda.aitools core + launchd services.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  templates = ./../templates;
  isDarwin = true;
  cdamuxPkg = pkgs.callPackage ../../cdamux/nix/package.nix {};
  graphifyPkg = pkgs.callPackage ./graphify-package.nix {};
  coolifyPkg = pkgs.callPackage ./coolify-cli-package.nix {};
  moshiHookFor = version:
    pkgs.callPackage ./moshi-hook-package.nix {inherit version;};
in {
  _module.args = {
    inherit templates cdamuxPkg graphifyPkg coolifyPkg moshiHookFor isDarwin;
  };

  imports = [
    ./module-core.nix
    ./darwin-services.nix
    (lib.mkRenamedOptionModule ["cda" "aitools" "ccmux" "projectsRoot"] ["cda" "aitools" "cdamux" "projectsRoot"])
    (lib.mkRenamedOptionModule ["cda" "aitools" "ccmuxd" "enable"] ["cda" "aitools" "cdamuxd" "enable"])
    (lib.mkRenamedOptionModule ["cda" "aitools" "darwin" "ccmuxdPath"] ["cda" "aitools" "darwin" "cdamuxdPath"])
  ];
}
