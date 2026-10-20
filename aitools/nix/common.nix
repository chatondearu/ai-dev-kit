# Shared option schema for cda.aitools (imported by module.nix).
# Not a standalone NixOS/darwin module — call with { lib, ... }.
{ lib }:
{
  enable = lib.mkEnableOption "cda.aitools companion (experimental tmux/ccmux wiring)";

  configRoot = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    description = ''
      Absolute path for aitools configs/state (e.g. /cda/aitools).
      When null, runtime falls back to CDA_AITOOLS_HOME or ~/.config/cda-aitools.
    '';
  };

  repoPath = lib.mkOption {
    type = lib.types.str;
    default = "{{home}}/dev/chatondearu/ai-dev-kit";
    description = "Path to the ai-dev-kit checkout (templates live under aitools/templates).";
  };

  installMethod = lib.mkOption {
    type = lib.types.enum [ "auto" "nix" "brew" "none" ];
    default = "auto";
    description = ''
      How to obtain session tools (tmux always via Nix when enablePackages;
      ccmux: auto prefers Nix then Darwin brew fallback; nix = Nix only;
      brew = Homebrew on Darwin; none = configs only).
    '';
  };

  enablePackages = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = "When true, add tmux (and ccmux if installMethod allows) to system packages.";
  };
}
