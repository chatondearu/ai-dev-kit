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
  ccmuxPkg = pkgs.ccmux or null;
  moshiHookFor = version:
    pkgs.callPackage ./moshi-hook-package.nix {inherit version;};
in {
  _module.args = {
    inherit templates ccmuxPkg moshiHookFor isDarwin;
  };

  imports = [
    ./module-core.nix
    ./darwin-services.nix
  ];
}
