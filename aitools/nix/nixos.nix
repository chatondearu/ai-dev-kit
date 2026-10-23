# NixOS entry: cda.aitools core + linux services.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  templates = ./../templates;
  isDarwin = pkgs.stdenv.isDarwin;
  ccmuxPkg = pkgs.ccmux or null;
  moshiHookFor = version:
    pkgs.callPackage ./moshi-hook-package.nix {inherit version;};
in {
  _module.args = {
    inherit templates ccmuxPkg moshiHookFor isDarwin;
  };

  imports = [
    ./module-core.nix
    ./linux-services.nix
  ];
}
