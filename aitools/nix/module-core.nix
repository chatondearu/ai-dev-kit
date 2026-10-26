# Shared cda.aitools: options, packages, maid deploy, assertions.
{
  config,
  lib,
  pkgs,
  templates,
  cdamuxPkg,
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
            || !(config.cda.aitools.cdamuxd.enable || config.cda.aitools.moshiHook.enable)
            || config.cda.aitools.user != null;
          message = "cda.aitools.user must be set when cdamuxd.enable or moshiHook.enable.";
        }
        {
          assertion =
            !config.cda.aitools.enable
            || !config.cda.aitools.cdamuxd.enable
            || cdamuxPkg != null
            || (isDarwin && config.cda.aitools.darwin.cdamuxdPath != "");
          message = "cdamuxd: need in-tree cdamux package or cda.aitools.darwin.cdamuxdPath.";
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
            || config.cda.aitools.cdamux.projectsRoot != "";
          message = "cda.aitools.cdamux.projectsRoot required when deploy.enable.";
        }
        {
          assertion =
            !config.cda.aitools.enable
            || !config.cda.aitools.tmux.persist.enable
            || config.cda.aitools.deploy.enable;
          message = "cda.aitools.tmux.persist requires deploy.enable.";
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
          && (a.installMethod == "nix" || a.installMethod == "auto")
          && cdamuxPkg != null
        ) [cdamuxPkg]
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
        && config.cda.aitools.installMethod == "brew"
      ) {
        "cda-aitools/CDAMUX_BREW.txt".text = ''
          cda.aitools: installMethod=brew — build cdamux from ai-dev-kit:
            cd ai-dev-kit/cdamux && cargo install --path crates/cdamux-cli --path crates/cdamuxd
          Or use installMethod = "nix" / "auto" (default) for the in-tree package.
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
          projectsRoot = config.cda.aitools.cdamux.projectsRoot;
          tmuxClipboard = config.cda.aitools.tmux.clipboard;
          tmuxPersist = config.cda.aitools.tmux.persist;
        };
    })
  ];
}
