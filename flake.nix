{
  description = "ai-dev-kit — shareable AI/dev config (Cursor skills, rules, agents, plugins) for Cursor/Claude/Codex/VSCode, delivered via nix-maid or install.sh";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nix-maid.url = "github:viperML/nix-maid";
  };

  outputs = { self, nixpkgs, nix-maid }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      # Importable maid declarations: import ./nix/maid.nix { repoPath = "…"; }
      lib.maidConfig = import ./nix/maid.nix;

      # Standalone nix-maid config. Install with:
      #   nix-env -if <path> && activate
      # or build/inspect with `nix build .#default`.
      packages = forAllSystems (pkgs: rec {
        default = nix-maid pkgs (import ./nix/maid.nix { });
        activate = default;
      });

      # NixOS module. In configuration.nix:
      #   imports = [ inputs.ai-dev-kit.nixosModules.default ];
      #   aiDevKit = { enable = true; user = "chaton"; repoPath = "{{home}}/dev/chatondearu/ai-dev-kit"; };
      nixosModules.default = { config, lib, ... }:
        let cfg = config.aiDevKit;
        in {
          imports = [ nix-maid.nixosModules.default ];

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
              type = lib.types.listOf (lib.types.enum [ "cursor" "claude" "opencode" "agents" ]);
              default = [ "cursor" ];
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

      formatter = forAllSystems (pkgs: pkgs.nixpkgs-fmt);
    };
}
