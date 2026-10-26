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
  cdamuxPkg = pkgs.callPackage ../../cdamux/nix/package.nix {};
  moshiHookFor = version:
    pkgs.callPackage ./moshi-hook-package.nix {inherit version;};
in {
  _module.args = {
    inherit templates cdamuxPkg moshiHookFor isDarwin;
  };

  imports = [
    ./module-core.nix
    ./linux-services.nix
    (lib.mkRenamedOptionModule ["cda" "aitools" "ccmux" "projectsRoot"] ["cda" "aitools" "cdamux" "projectsRoot"])
    (lib.mkRenamedOptionModule ["cda" "aitools" "ccmuxd" "enable"] ["cda" "aitools" "cdamuxd" "enable"])
    (lib.mkRenamedOptionModule ["cda" "aitools" "darwin" "ccmuxdPath"] ["cda" "aitools" "darwin" "cdamuxdPath"])
  ];
}
