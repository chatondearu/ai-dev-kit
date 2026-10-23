# NixOS: mosh + systemd user units (no config.* at module top level).
{
  config,
  lib,
  pkgs,
  ccmuxPkg,
  moshiHookFor,
  ...
}:
{
  config = lib.mkMerge [
    (lib.mkIf (config.cda.aitools.enable && config.cda.aitools.mosh.enable) {
      programs.mosh.enable = true;
    })
    (lib.mkIf (
      config.cda.aitools.enable
      && config.cda.aitools.systemdUser.enable
      && config.cda.aitools.ccmuxd.enable
      && ccmuxPkg != null
    ) (
      let
        cond =
          lib.optionalAttrs (config.cda.aitools.systemdUser.conditionUser != null) {
            unitConfig.ConditionUser = config.cda.aitools.systemdUser.conditionUser;
          };
      in {
        systemd.user.services.ccmuxd = {
          description = "ccmux daemon (AI agent session supervisor)";
          after = ["default.target"];
          wantedBy = ["default.target"];
          serviceConfig = {
            ExecStart = "${ccmuxPkg}/bin/ccmuxd";
            Restart = "on-failure";
            RestartSec = 3;
            Environment = [
              "PATH=%h/.local/bin:/run/current-system/sw/bin:/usr/bin:/bin"
            ];
          };
        }
        // cond;
      }
    ))
    (lib.mkIf (
      config.cda.aitools.enable
      && config.cda.aitools.systemdUser.enable
      && config.cda.aitools.moshiHook.enable
    ) (
      let
        a = config.cda.aitools;
        moshiHookPkg = moshiHookFor a.moshiHook.version;
        hookTargetsArg = lib.concatStringsSep "," a.moshiHook.hookTargets;
        tokenFileArg = lib.escapeShellArg (
          if a.moshiHook.pairingTokenFile != null
          then toString a.moshiHook.pairingTokenFile
          else ""
        );
        cond =
          lib.optionalAttrs (a.systemdUser.conditionUser != null) {
            unitConfig.ConditionUser = a.systemdUser.conditionUser;
          };
        pairScript = pkgs.writeShellScript "moshi-hook-pair" ''
          set -euo pipefail
          export MOSHI_HOOK_SKIP_FIRST_RUN=1
          if ${moshiHookPkg}/bin/moshi-hook status --json | ${pkgs.jq}/bin/jq -e '.paired == true' >/dev/null; then
            exit 0
          fi
          token_file=${tokenFileArg}
          if [ -z "$token_file" ] || [ ! -f "$token_file" ]; then
            echo "moshi-hook: not paired — set pairingTokenFile or run aitools/scripts/moshi-hook-bootstrap.sh" >&2
            exit 0
          fi
          ${moshiHookPkg}/bin/moshi-hook pair --token "$(<"$token_file")" --store file
        '';
        installScript = pkgs.writeShellScript "moshi-hook-install-hooks" ''
          set -euo pipefail
          export MOSHI_HOOK_SKIP_FIRST_RUN=1
          ${moshiHookPkg}/bin/moshi-hook install --target ${lib.escapeShellArg hookTargetsArg}
        '';
      in {
        systemd.user.services.moshi-hook-pair = {
          description = "Pair moshi-hook with Moshi (when token file is present)";
          before = ["moshi-hook-install.service" "moshi-hook.service"];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = pairScript;
          };
        }
        // cond;

        systemd.user.services.moshi-hook-install = {
          description = "Install Moshi agent hooks (${hookTargetsArg})";
          after = ["moshi-hook-pair.service"];
          before = ["moshi-hook.service"];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = installScript;
          };
        }
        // cond;

        systemd.user.services.moshi-hook = {
          description = "Moshi agent hook daemon (mobile push / approvals)";
          after = ["network-online.target" "moshi-hook-install.service" "moshi-hook-pair.service"];
          requires = ["moshi-hook-install.service"];
          wants = ["network-online.target"];
          wantedBy = ["default.target"];
          serviceConfig = {
            ExecStart = "${moshiHookPkg}/bin/moshi-hook serve";
            Restart = "on-failure";
            RestartSec = 3;
            Environment = ["MOSHI_HOOK_SKIP_FIRST_RUN=1"];
          };
        }
        // cond;
      }
    ))
  ];
}
