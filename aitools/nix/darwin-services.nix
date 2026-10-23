# nix-darwin: launchd user agents (no config.* at module top level).
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
    (lib.mkIf (config.cda.aitools.enable && config.cda.aitools.ccmuxd.enable) (
      let
        a = config.cda.aitools;
        ccmuxBin =
          if ccmuxPkg != null
          then "${ccmuxPkg}/bin/ccmuxd"
          else a.darwin.ccmuxdPath;
      in {
        launchd.user.agents.ccmuxd = {
          serviceConfig = {
            ProgramArguments = [ccmuxBin];
            RunAtLoad = true;
            KeepAlive = true;
            EnvironmentVariables = {
              PATH = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin";
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
