# nix-darwin: launchd user agents (no config.* at module top level).
{
  config,
  lib,
  pkgs,
  cdamuxPkg,
  moshiHookFor,
  ...
}:
{
  config = lib.mkMerge [
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
