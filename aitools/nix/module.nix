# cda.aitools module — usable as nixosModules.cdaAitools and darwinModules.cdaAitools.
# Experimental. Shared options; package backends differ by platform.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.cda.aitools;
  optionsSchema = import ./common.nix { inherit lib; };
  isDarwin = pkgs.stdenv.isDarwin;
  # ccmux is often missing from nixpkgs; treat Nix install as best-effort.
  ccmuxPkg = pkgs.ccmux or null;
  wantTmux = cfg.enable && cfg.enablePackages && cfg.installMethod != "none";
  wantCcmuxNix =
    cfg.enable && cfg.enablePackages
    && (cfg.installMethod == "nix"
      || (cfg.installMethod == "auto" && ccmuxPkg != null));
  wantCcmuxBrewHint =
    cfg.enable && cfg.enablePackages && isDarwin
    && (cfg.installMethod == "brew"
      || (cfg.installMethod == "auto" && ccmuxPkg == null));
  packages =
    lib.optionals wantTmux [ pkgs.tmux ]
    ++ lib.optionals (wantCcmuxNix && ccmuxPkg != null) [ ccmuxPkg ];
in
{
  options.cda.aitools = optionsSchema;

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.installMethod != "brew" || isDarwin;
        message = "cda.aitools.installMethod = \"brew\" is only valid on Darwin (use nix|auto|none on NixOS).";
      }
    ];

    # NixOS
    environment.systemPackages = lib.mkIf (packages != [ ]) packages;

    # Expose resolved root for activation scripts / docs consumers.
    environment.variables = lib.mkIf (cfg.configRoot != null) {
      CDA_AITOOLS_HOME = cfg.configRoot;
    };

    # Hint file when brew fallback is expected (Darwin auto without nixpkgs ccmux).
    environment.etc = lib.mkIf wantCcmuxBrewHint {
      "cda-aitools/BREW_CCMUX.txt".text = ''
        cda.aitools: ccmux not available from nixpkgs on this evaluation.
        Install with: brew install ccmux
        Then: ccmux setup
        Configs: see ai-dev-kit aitools/templates/ (copy into $CDA_AITOOLS_HOME).
      '';
    };
  };
}
