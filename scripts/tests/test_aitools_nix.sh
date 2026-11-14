#!/usr/bin/env bash
# aitools nix hygiene + graphify option schema eval (network only for optional package build).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "== bash -n bootstrap scripts =="
bash -n "$ROOT/aitools/scripts/moshi-hook-bootstrap.sh"
bash -n "$ROOT/aitools/scripts/graphify-activate.sh"

if ! command -v nix >/dev/null; then
  echo "SKIP: nix not found"
  exit 0
fi

echo "== graphify option schema =="
nix eval --impure --expr "
  let
    flake = builtins.getFlake \"path:$ROOT\";
    pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
    lib = pkgs.lib;
    schema = import $ROOT/aitools/nix/common.nix { inherit lib; };
  in
    assert schema ? graphify;
    assert schema.graphify ? enable;
    assert schema.graphify ? installMethod;
    assert schema.graphify ? claudeConfigDir;
    assert !(schema.graphify ? hooks);
    assert schema ? foamHome;
    assert schema ? coolify;
    assert schema.coolify ? enable;
    assert schema.coolify ? installMethod;
    assert schema.coolify ? baseUrl;
    \"ok\"
"

echo "== graphify module on/off (nixos + darwin) =="
# Stub host options so we can eval nixosModules/darwinModules without a full
# NixOS or nix-darwin system (flake has no nix-darwin input).
nix eval --impure --expr "
  let
    flake = builtins.getFlake \"path:$ROOT\";
    lib = flake.inputs.nixpkgs.lib;
    system = builtins.currentSystem;
    pkgs = import flake.inputs.nixpkgs {
      inherit system;
      config.allowUnfreePredicate = pkg: pkg.pname == \"moshi-hook\";
    };
    stub = {
      options = {
        assertions = lib.mkOption { type = lib.types.listOf lib.types.unspecified; default = []; };
        warnings = lib.mkOption { type = lib.types.listOf lib.types.str; default = []; };
        environment.systemPackages = lib.mkOption { type = lib.types.listOf lib.types.package; default = []; };
        environment.variables = lib.mkOption {
          type = lib.types.attrsOf (lib.types.nullOr (lib.types.oneOf [ lib.types.str (lib.types.listOf lib.types.str) ]));
          default = {};
        };
        environment.etc = lib.mkOption { type = lib.types.attrsOf lib.types.unspecified; default = {}; };
        users.users = lib.mkOption {
          type = lib.types.attrsOf (lib.types.submodule {
            options = {
              maid = lib.mkOption { type = lib.types.unspecified; default = null; };
              home = lib.mkOption { type = lib.types.str; default = \"/home/tester\"; };
            };
          });
          default = {};
        };
        systemd.user.services = lib.mkOption { type = lib.types.attrsOf lib.types.unspecified; default = {}; };
        systemd.tmpfiles.rules = lib.mkOption { type = lib.types.listOf lib.types.str; default = []; };
        programs.mosh.enable = lib.mkOption { type = lib.types.bool; default = false; };
        launchd.user.agents = lib.mkOption { type = lib.types.attrsOf lib.types.unspecified; default = {}; };
      };
      config._module.args.pkgs = pkgs;
    };
    evalPlatform = { isDarwin, graphifyEnable }:
      let
        mod = if isDarwin then flake.darwinModules.cdaAitools else flake.nixosModules.cdaAitools;
        eval = lib.evalModules {
          modules = [
            stub
            mod
            {
              cda.aitools = {
                enable = true;
                user = \"tester\";
                enablePackages = true;
                installMethod = \"none\";
                graphify.enable = graphifyEnable;
                graphify.installMethod = \"nix\";
                graphify.claudeConfigDir = \"/cfg/claude\";
              };
            }
          ];
        };
        failed = builtins.filter (a: !(a.assertion)) eval.config.assertions;
        pkgsNames = map (p: p.pname or p.name or \"?\") eval.config.environment.systemPackages;
        hasGraphifyPkg = builtins.any (n: lib.hasInfix \"graphify\" n) pkgsNames;
        hasEnv = (eval.config.environment.variables.CDA_GRAPHIFY or null) == \"1\";
        hasLinuxUnit = eval.config.systemd.user.services ? cda-graphify-install;
        hasDarwinUnit = eval.config.launchd.user.agents ? cda-graphify-install;
        hasUnit = if isDarwin then hasDarwinUnit else hasLinuxUnit;
        # nix-darwin types ProgramArguments as listOf str (the stub does not).
        darwinArgsAreStrings = !hasDarwinUnit
          || builtins.all builtins.isString eval.config.launchd.user.agents.cda-graphify-install.serviceConfig.ProgramArguments;
      in
        assert failed == [];
        assert hasEnv == graphifyEnable;
        assert hasGraphifyPkg == graphifyEnable;
        assert hasUnit == graphifyEnable;
        assert hasLinuxUnit == (!isDarwin && graphifyEnable);
        assert hasDarwinUnit == (isDarwin && graphifyEnable);
        assert darwinArgsAreStrings;
        \"ok\";
    # Units do not inherit the login shell: activation must carry CLAUDE_CONFIG_DIR.
    activateScript = import $ROOT/aitools/nix/graphify-activation.nix {
      inherit pkgs lib;
      graphifyPkg = null;
      pathTail = \"/usr/bin:/bin\";
      a.graphify = { installMethod = \"none\"; claudeConfigDir = \"/cfg/claude\"; };
    };
  in
    assert lib.hasInfix \"CLAUDE_CONFIG_DIR=/cfg/claude\" activateScript.text;
    assert evalPlatform { isDarwin = false; graphifyEnable = false; } == \"ok\";
    assert evalPlatform { isDarwin = false; graphifyEnable = true; } == \"ok\";
    assert evalPlatform { isDarwin = true; graphifyEnable = false; } == \"ok\";
    assert evalPlatform { isDarwin = true; graphifyEnable = true; } == \"ok\";
    \"ok\"
"

echo "== cdamux hooks module on/off (nixos + darwin) =="
# Same host-option stub idea as above; off by default, unit only when enabled.
nix eval --impure --expr "
  let
    flake = builtins.getFlake \"path:$ROOT\";
    lib = flake.inputs.nixpkgs.lib;
    pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
    any = lib.mkOption { type = lib.types.attrsOf lib.types.unspecified; default = {}; };
    stub = {
      options = {
        assertions = lib.mkOption { type = lib.types.listOf lib.types.unspecified; default = []; };
        environment.systemPackages = lib.mkOption { type = lib.types.listOf lib.types.package; default = []; };
        environment.variables = any;
        environment.etc = any;
        users.users = any;
        systemd.user.services = any;
        systemd.tmpfiles.rules = lib.mkOption { type = lib.types.listOf lib.types.str; default = []; };
        programs.mosh.enable = lib.mkOption { type = lib.types.bool; default = false; };
        launchd.user.agents = any;
      };
      config._module.args.pkgs = pkgs;
    };
    evalPlatform = { isDarwin, hooks }:
      let
        mod = if isDarwin then flake.darwinModules.cdaAitools else flake.nixosModules.cdaAitools;
        cfg = (lib.evalModules {
          modules = [ stub mod {
            cda.aitools = { enable = true; user = \"tester\"; installMethod = \"none\"; }
              // lib.optionalAttrs hooks { cdamux.hooks = { enable = true; claudeConfigDir = \"/cfg/claude\"; }; };
          } ];
        }).config;
        units = if isDarwin then cfg.launchd.user.agents else cfg.systemd.user.services;
      in
        assert builtins.filter (a: !(a.assertion)) cfg.assertions == [];
        assert (units ? cda-cdamux-hooks) == hooks;
        # nix-darwin types ProgramArguments as listOf str (the stub does not).
        assert !hooks || !isDarwin || builtins.all builtins.isString units.cda-cdamux-hooks.serviceConfig.ProgramArguments;
        \"ok\";
    # Units do not inherit the login shell: activation must carry CLAUDE_CONFIG_DIR.
    script = import $ROOT/aitools/nix/cdamux-hooks-activation.nix {
      inherit pkgs lib;
      cdamuxPkg = \"/pkg\";
      a.cdamux.hooks.claudeConfigDir = \"/cfg/claude\";
    };
  in
    assert lib.hasInfix \"CLAUDE_CONFIG_DIR=/cfg/claude\" script.text;
    assert lib.hasInfix \"/pkg/bin/cdamux-hooks wire\" script.text;
    assert evalPlatform { isDarwin = false; hooks = false; } == \"ok\";
    assert evalPlatform { isDarwin = false; hooks = true; } == \"ok\";
    assert evalPlatform { isDarwin = true; hooks = false; } == \"ok\";
    assert evalPlatform { isDarwin = true; hooks = true; } == \"ok\";
    \"ok\"
"

<<<<<<< HEAD
echo "== cdamux navBind schema + maid template =="
nix eval --impure --expr "
  let
    flake = builtins.getFlake \"path:$ROOT\";
    pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
    lib = pkgs.lib;
    schema = import $ROOT/aitools/nix/common.nix { inherit lib; };
    templates = $ROOT/aitools/templates;
    mkDeploy = nav: import $ROOT/aitools/nix/maid-deploy.nix {
      inherit lib pkgs templates;
      projectsRoot = \"/dev\";
      tmuxClipboard = \"none\";
      tmuxPersist = { enable = false; saveInterval = 15; };
      navBind = { enable = nav; };
    };
    offSrc = (mkDeploy false).file.xdg_config.\"cda-aitools/tmux.conf\".source;
    onSrc = (mkDeploy true).file.xdg_config.\"cda-aitools/tmux.conf\".source;
    offText = builtins.readFile offSrc;
    onText = builtins.readFile onSrc;
    bind = \"bind-key -n M-n run-shell \\\"cdamux nav --client '#{client_name}'\\\"\";
  in
    assert schema.cdamux ? navBind;
    assert schema.cdamux.navBind ? enable;
    assert !(lib.hasInfix \"M-n\" offText);
    assert lib.hasInfix bind onText;
=======
echo "== coolify module on/off =="
nix eval --impure --expr "
  let
    flake = builtins.getFlake \"path:$ROOT\";
    lib = flake.inputs.nixpkgs.lib;
    pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
    any = lib.mkOption { type = lib.types.attrsOf lib.types.unspecified; default = {}; };
    stub = {
      options = {
        assertions = lib.mkOption { type = lib.types.listOf lib.types.unspecified; default = []; };
        environment.systemPackages = lib.mkOption { type = lib.types.listOf lib.types.package; default = []; };
        environment.variables = any;
        environment.etc = any;
        users.users = any;
        systemd.user.services = any;
        systemd.tmpfiles.rules = lib.mkOption { type = lib.types.listOf lib.types.str; default = []; };
        programs.mosh.enable = lib.mkOption { type = lib.types.bool; default = false; };
        launchd.user.agents = any;
      };
      config._module.args.pkgs = pkgs;
    };
    evalCoolify = enable:
      let
        cfg = (lib.evalModules {
          modules = [ stub flake.nixosModules.cdaAitools {
            cda.aitools = {
              enable = true;
              user = \"tester\";
              installMethod = \"none\";
              coolify.enable = enable;
              coolify.installMethod = \"nix\";
              coolify.baseUrl = if enable then \"https://coolify.example.com\" else null;
            };
          } ];
        }).config;
        names = map (p: p.pname or p.name or \"?\") cfg.environment.systemPackages;
        hasCli = builtins.any (n: lib.hasInfix \"coolify\" n) names;
        hasUrl = (cfg.environment.variables.COOLIFY_BASE_URL or null) == \"https://coolify.example.com\";
      in
        assert builtins.filter (a: !(a.assertion)) cfg.assertions == [];
        assert hasCli == enable;
        assert hasUrl == enable;
        \"ok\";
  in
    assert evalCoolify false == \"ok\";
    assert evalCoolify true == \"ok\";
>>>>>>> 9cd8898 (feat(aitools): package Coolify CLI with opt-in cda.aitools.coolify)
    \"ok\"
"

echo "== graphify package parses =="
nix-instantiate --parse "$ROOT/aitools/nix/graphify-package.nix" >/dev/null
nix-instantiate --parse "$ROOT/aitools/nix/graphify-activation.nix" >/dev/null
nix-instantiate --parse "$ROOT/aitools/nix/coolify-cli-package.nix" >/dev/null

echo "== graphify-activate isolated (symlink CLAUDE.md + throwaway cwd) =="
# Never touch live maintainer config (AGENTS.md gotcha).
FAKE="$(mktemp -d "${TMPDIR:-/tmp}/cda-test-graphify-activate.XXXXXX")"
KIT="$FAKE/kit"
mkdir -p "$KIT/claude" "$FAKE/home" "$FAKE/xdg" "$FAKE/cursor" "$FAKE/cwd"
printf '# kit CLAUDE.md\n' >"$KIT/claude/CLAUDE.md"
KIT_BEFORE="$(cat "$KIT/claude/CLAUDE.md")"
printf '# cwd CLAUDE.md\n' >"$FAKE/cwd/CLAUDE.md"
CWD_BEFORE="$(cat "$FAKE/cwd/CLAUDE.md")"
mkdir -p "$FAKE/claude"
ln -s "$KIT/claude/CLAUDE.md" "$FAKE/claude/CLAUDE.md"
GF_BIN="$(nix build --no-link --print-out-paths "$ROOT#graphify")/bin"
(
  export HOME="$FAKE/home"
  export CLAUDE_CONFIG_DIR="$FAKE/claude"
  export XDG_CONFIG_HOME="$FAKE/xdg"
  export CURSOR_HOME="$FAKE/cursor"
  export PATH="$GF_BIN:$PATH"
  cd "$FAKE/cwd"
  bash "$ROOT/aitools/scripts/graphify-activate.sh"
)
test -f "$FAKE/claude/skills/graphify/SKILL.md"
test -L "$FAKE/claude/CLAUDE.md"
test "$(cat "$KIT/claude/CLAUDE.md")" = "$KIT_BEFORE"
test "$(cat "$FAKE/cwd/CLAUDE.md")" = "$CWD_BEFORE"
rm -rf "$FAKE"

# Optional heavy build (CDN / PyPI); skip unless CDA_TEST_AITOOLS_BUILD=1
if [ "${CDA_TEST_AITOOLS_BUILD:-0}" = "1" ]; then
  system="$(nix eval --impure --raw --expr builtins.currentSystem)"
  echo "== build moshi-hook package ($system) =="
  nix build --no-link "$ROOT#packages.$system.moshi-hook" -L
  echo "== build graphify package ($system) =="
  nix build --no-link "$ROOT#packages.$system.graphify" -L
fi

echo "ok"
