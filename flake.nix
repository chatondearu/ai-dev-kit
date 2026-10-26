{
  description = "ai-dev-kit — shareable AI/dev config (Cursor skills, rules, agents, plugins) for Cursor/Claude/Codex/VSCode, delivered via nix-maid or install.sh";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-maid.url = "github:viperML/nix-maid";
  };

  outputs = {self, nixpkgs, nix-maid}:
    let
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (
          system:
            let
              pkgs = import nixpkgs {
                inherit system;
                config.allowUnfreePredicate = pkg: pkg.pname == "moshi-hook";
              };
            in
            f pkgs
        );
    in {
      lib.maidConfig = import ./nix/maid.nix;

      packages = forAllSystems (pkgs: rec {
        default = nix-maid pkgs (import ./nix/maid.nix {});
        activate = default;
        moshi-hook = pkgs.callPackage ./aitools/nix/moshi-hook-package.nix {};
        cdamux = pkgs.callPackage ./cdamux/nix/package.nix {};
      });

      nixosModules.default = {config, lib, ...}: let
        cfg = config.aiDevKit;
      in {
        imports = [nix-maid.nixosModules.default];

        options.aiDevKit = {
          enable = lib.mkEnableOption "ai-dev-kit agent assets (skills, rules, agents, plugins)";
          user = lib.mkOption {
            type = lib.types.str;
            description = "User to install the assets for.";
          };
          repoPath = lib.mkOption {
            type = lib.types.str;
            default = "{{home}}/dev/chatondearu/ai-dev-kit";
            description = "Path to the checked-out ai-dev-kit repo ({{home}} mustache allowed).";
          };
          tools = lib.mkOption {
            type = lib.types.listOf (lib.types.enum ["cursor" "claude" "opencode" "agents"]);
            default = ["cursor"];
            description = "Agents to feed the shared skills into.";
          };
        };

        config = lib.mkIf cfg.enable {
          users.users.${cfg.user}.maid =
            let
              dirFor = t: {
                cursor = ".cursor/skills";
                claude = ".claude/skills";
                opencode = ".config/opencode/skills";
                agents = ".agents/skills";
              }.${t};
            in
            import ./nix/maid.nix {
              inherit (cfg) repoPath;
              skillDirs = map dirFor cfg.tools;
            };
        };
      };

      nixosModules.cdaAitools = import ./aitools/nix/nixos.nix;
      darwinModules.cdaAitools = import ./aitools/nix/darwin.nix;

      formatter = forAllSystems (pkgs: pkgs.nixpkgs-fmt);
    };
}
