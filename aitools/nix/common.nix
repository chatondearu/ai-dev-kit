# Shared option schema for cda.aitools (imported by module-core.nix).
{lib}:
{
  enable = lib.mkEnableOption "cda.aitools companion (experimental tmux/cdamux/Moshi wiring)";

  user = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    description = ''
      Login user for maid deploy and platform services.
      Required when cdamuxd.enable, moshiHook.enable, or deploy.enable.
    '';
  };

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
    type = lib.types.enum ["auto" "nix" "brew" "none"];
    default = "auto";
    description = ''
      How to obtain session tools (tmux always via Nix when enablePackages;
      cdamux: auto/nix use the in-tree Nix package; brew = legacy manual install;
      none = configs only).
    '';
  };

  enablePackages = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = "When true, add tmux and cdamux (when installMethod allows) to system packages.";
  };

  deploy = {
    enable = lib.mkEnableOption "deploy tmux/cdamux configs via nix-maid into the user home";
  };

  cdamux = {
    projectsRoot = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "Host-specific primary [[projects.roots]] path in config.toml (required when deploy.enable).";
      example = "/hdd/dev";
    };
  };

  tmux = {
    clipboard = lib.mkOption {
      type = lib.types.enum ["darwin" "wayland" "none"];
      default = "darwin";
      description = "Extra ~/.tmux.conf binds on top of the shared template.";
    };

    persist = {
      enable = lib.mkEnableOption "tmux-resurrect + continuum (recover sessions after reboot)";

      saveInterval = lib.mkOption {
        type = lib.types.int;
        default = 15;
        description = "Continuum auto-save interval in minutes.";
      };
    };
  };

  darwin = {
    cdamuxdPath = lib.mkOption {
      type = lib.types.str;
      default = "/opt/homebrew/bin/cdamuxd";
      description = "cdamuxd binary when not installed from the Nix cdamux package.";
    };
  };

  mosh = {
    enable = lib.mkEnableOption "mosh client and mosh-server (mobile shell, not Moshi iOS)";
  };

  cdamuxd = {
    enable = lib.mkEnableOption "cdamuxd user daemon (systemd on NixOS, launchd on darwin)";
  };

  moshiHook = {
    enable = lib.mkEnableOption "moshi-hook daemon and Cursor/agent hooks for Moshi mobile";

    version = lib.mkOption {
      type = lib.types.str;
      default = "v0.4.36";
      description = "Pinned moshi-hook release on cdn.getmoshi.app.";
    };

    hookTargets = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = ["cursor"];
      description = "Agents passed to moshi-hook install --target (comma-separated).";
      example = ["cursor"];
    };

    pairingTokenFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Host path to Moshi pairing token (e.g. agenix secret). When set and the host
        is not yet paired, moshi-hook-pair runs at login. Omit to pair manually once.
      '';
    };
  };

  systemdUser = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "When true, declare systemd user units for cdamuxd/moshi-hook on NixOS.";
    };

    conditionUser = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Optional systemd ConditionUser (e.g. \"!dms-greeter\").";
      example = "!dms-greeter";
    };
  };
}
