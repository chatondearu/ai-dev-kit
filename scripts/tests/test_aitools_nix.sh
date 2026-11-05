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

echo "== graphify package parses =="
nix-instantiate --parse "$ROOT/aitools/nix/graphify-package.nix" >/dev/null
nix-instantiate --parse "$ROOT/aitools/nix/graphify-activation.nix" >/dev/null

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
