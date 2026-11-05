# nix-darwin: launchd user agents (no config.* at module top level).
{
  config,
  lib,
  pkgs,
  cdamuxPkg,
  graphifyPkg,
  moshiHookFor,
  ...
}:
{
  config = lib.mkMerge [
    (lib.mkIf (
      config.cda.aitools.enable
      && config.cda.aitools.graphify.enable
      && config.cda.aitools.graphify.installMethod != "none"
      && config.cda.aitools.user != null
    ) (
      let
        a = config.cda.aitools;
        activateExec = import ./graphify-activation.nix {
          inherit pkgs lib graphifyPkg a;
          pathTail = "/run/current-system/sw/bin:/opt/homebrew/bin:/usr/bin:/bin:$PATH";
        };
      in {
        launchd.user.agents.cda-graphify-install = {
          serviceConfig = {
            ProgramArguments = [activateExec];
            RunAtLoad = true;
            KeepAlive = false;
          };
        };
      }
    ))
    (lib.mkIf (config.cda.aitools.enable && config.cda.aitools.cdamuxd.enable) (
      let
        a = config.cda.aitools;
        cdamuxdBin =
          if cdamuxPkg != null
          then "${cdamuxPkg}/bin/cdamuxd"
          else a.darwin.cdamuxdPath;
      in {
        launchd.user.agents.cdamuxd = {
          serviceConfig = {
            ProgramArguments = [cdamuxdBin];
            RunAtLoad = true;
            # Relaunch on failure only: cdamuxd exits 0 when listen_tailnet is false.
            KeepAlive = { SuccessfulExit = false; };
            # Nix tmux lives under /run/current-system/sw/bin — without it
            # cdamuxd cannot poll and session STATE stays unknown.
            EnvironmentVariables = {
              PATH = "/run/current-system/sw/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/Users/${a.user}/.local/bin:/usr/bin:/bin";
            };
          };
        };
      }
    ))
    (lib.mkIf (config.cda.aitools.enable && config.cda.aitools.moshiHook.enable) (
      let
        moshiHookPkg = moshiHookFor config.cda.aitools.moshiHook.version;
      in {
        launchd.user.agents.moshi-hook = {
          serviceConfig = {
            ProgramArguments = ["${moshiHookPkg}/bin/moshi-hook" "serve"];
            RunAtLoad = true;
            KeepAlive = true;
            EnvironmentVariables = {
              MOSHI_HOOK_SKIP_FIRST_RUN = "1";
            };
          };
        };
      }
    ))
  ];
}
