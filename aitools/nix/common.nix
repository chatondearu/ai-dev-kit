# Shared option schema for cda.aitools (imported by module-core.nix).
{lib}:
{
  enable = lib.mkEnableOption "cda.aitools companion (experimental tmux/cdamux/Moshi wiring)";

  user = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    description = ''
      Login user for maid deploy and platform services.
      Required when cdamuxd.enable, moshiHook.enable, deploy.enable, or
      graphify.enable with installMethod != "none".
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

  foamHome = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    description = ''
      Absolute path exported as CDA_FOAM_HOME (shared project memory root for
      skills such as cda-floorplan). Null leaves the variable unset.
    '';
    example = "/cda/foam";
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

    hooks = {
      enable = lib.mkEnableOption ''
        cdamux agent hooks (sub-agent ledger + session id capture) for Claude Code,
        Cursor and opencode: an activation oneshot runs `cdamux-hooks wire` at every
        rebuild/login, so hook updates follow the cdamux package. Running agents
        keep the old hooks until restarted (`cdamux restart --all`)'';

      claudeConfigDir = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = ''
          CLAUDE_CONFIG_DIR for the activation oneshot. launchd/systemd units do not
          inherit the login shell, so without it the hooks land in ~/.claude.
        '';
        example = "/cda/.claude";
      };
    };

    navBind = {
      enable = lib.mkEnableOption ''
        Opt-in Alt-N (M-n) → `cdamux nav` in the managed tmux.conf (ADR-014).
        Default off; requires deploy.enable. Portable: install.sh --cdamux-nav-bind'';
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
    enable = lib.mkEnableOption "cdamuxd user daemon (systemd on NixOS, launchd on darwin); only useful with `[daemon] listen_tailnet = true` in the cdamux config, otherwise the unit exits immediately";
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

  graphify = {
    enable = lib.mkEnableOption "Graphify code-graph CLI (PyPI graphifyy) + CDA_GRAPHIFY=1";

    installMethod = lib.mkOption {
      type = lib.types.enum ["auto" "nix" "uv" "none"];
      default = "auto";
      description = ''
        How to obtain graphify: nix = pinned PyPI derivation; uv = uv tool install
        graphifyy at activation; auto = prefer nix package when built into the
        module, else uv at activation; none = env flag only (manual install).
      '';
    };

    claudeConfigDir = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        CLAUDE_CONFIG_DIR for the activation oneshot. launchd/systemd units do not
        inherit the login shell, so without it the skill lands in ~/.claude.
      '';
      example = "/cda/.claude";
    };
  };

  coolify = {
    enable = lib.mkEnableOption ''
      Official Coolify CLI (flake package coolify-cli). Off by default; never
      wired from install.sh (ADR-001 / ADR-003). API token stays on the host
      (COOLIFY_TOKEN); run coolify-env-sync after export.
    '';

    installMethod = lib.mkOption {
      type = lib.types.enum ["auto" "nix" "none"];
      default = "auto";
      description = ''
        How to obtain coolify: nix/auto = in-tree pinned release binary when the
        package is available; none = configs/env only (manual install).
      '';
    };

    baseUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        Optional public Coolify instance URL exported as COOLIFY_BASE_URL.
        Do not put the API token in Nix — use host env COOLIFY_TOKEN + coolify-env-sync.
      '';
      example = "https://coolify.example.com";
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
