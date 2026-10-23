# Shared cda.aitools: options, packages, maid deploy, assertions.
{
  config,
  lib,
  pkgs,
  templates,
  ccmuxPkg,
  moshiHookFor,
  isDarwin,
  ...
}:
let
  optionsSchema = import ./common.nix {inherit lib;};
in {
  options.cda.aitools = optionsSchema;

  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion =
            !config.cda.aitools.enable
            || config.cda.aitools.installMethod != "brew"
            || isDarwin;
          message = "cda.aitools.installMethod = \"brew\" is only valid on Darwin.";
        }
        {
          assertion =
            !config.cda.aitools.enable
            || !(config.cda.aitools.ccmuxd.enable || config.cda.aitools.moshiHook.enable)
            || config.cda.aitools.user != null;
          message = "cda.aitools.user must be set when ccmuxd.enable or moshiHook.enable.";
        }
        {
          assertion =
            !config.cda.aitools.enable
            || !config.cda.aitools.ccmuxd.enable
            || ccmuxPkg != null
            || (isDarwin && config.cda.aitools.darwin.ccmuxdPath != "");
          message = "ccmuxd: need pkgs.ccmux (NixOS) or cda.aitools.darwin.ccmuxdPath (darwin + brew).";
        }
        {
          assertion =
            !config.cda.aitools.enable
            || !config.cda.aitools.deploy.enable
            || config.cda.aitools.user != null;
          message = "cda.aitools.user required when deploy.enable.";
        }
        {
          assertion =
            !config.cda.aitools.enable
            || !config.cda.aitools.deploy.enable
            || config.cda.aitools.ccmux.projectsRoot != "";
          message = "cda.aitools.ccmux.projectsRoot required when deploy.enable.";
        }
      ];
    }
    (lib.mkIf config.cda.aitools.enable {
      environment.systemPackages =
        let
          a = config.cda.aitools;
        in
        lib.optionals (a.enablePackages && a.installMethod != "none") [pkgs.tmux]
        ++ lib.optionals (
          a.enablePackages
          && (a.installMethod == "nix" || (a.installMethod == "auto" && ccmuxPkg != null))
          && ccmuxPkg != null
        ) [ccmuxPkg]
        ++ lib.optionals a.moshiHook.enable [
          (moshiHookFor a.moshiHook.version)
          pkgs.jq
        ];

      environment.variables = lib.mkIf (config.cda.aitools.configRoot != null) {
        CDA_AITOOLS_HOME = config.cda.aitools.configRoot;
      };

      environment.etc = lib.mkIf (
        config.cda.aitools.enablePackages
        && isDarwin
        && (config.cda.aitools.installMethod == "brew"
          || (config.cda.aitools.installMethod == "auto" && ccmuxPkg == null))
      ) {
        "cda-aitools/BREW_CCMUX.txt".text = ''
          cda.aitools: install ccmux with Homebrew:
            brew install skzv/tap/ccmux
          Or run: aitools/scripts/ccmux-align-mac.sh
          Configs deploy from ai-dev-kit aitools/templates when deploy.enable.
        '';
      };
    })
    (lib.mkIf (
      config.cda.aitools.enable
      && config.cda.aitools.deploy.enable
      && config.cda.aitools.user != null
    ) {
      users.users.${config.cda.aitools.user}.maid =
        import ./maid-deploy.nix {
          inherit lib pkgs templates;
          projectsRoot = config.cda.aitools.ccmux.projectsRoot;
          tmuxClipboard = config.cda.aitools.tmux.clipboard;
        };
    })
  ];
}
