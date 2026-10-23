# moshi-hook — Moshi agent daemon (not the iOS app). Pinned CDN release.
{
  lib,
  stdenv,
  fetchurl,
  version ? "v0.4.36",
}:
let
  ver = lib.removePrefix "v" version;
  assetFor = system:
    {
      x86_64-linux = {
        name = "Linux_x86_64";
        hash = "sha256-HWBuLFHgcCDCTHWyRsbRZaLIU5sOPVNs3HsqtF8JSYs=";
      };
      aarch64-linux = {
        name = "Linux_arm64";
        hash = "sha256-/gih3urPy1OQJkjqEJFDlWvFcUNUiEcP4r6rgSbxDlo=";
      };
      aarch64-darwin = {
        name = "Darwin_arm64";
        hash = "sha256-77pEaB3mTvDsZEos0mZ80pKSB4QcgXSQSqyuBetsZ7Y=";
      };
      x86_64-darwin = {
        name = "Darwin_x86_64";
        hash = "sha256-xZjwch2oRQcsxa0QCztnWvAWOBe7LkkalmK5ELwf0aU=";
      };
    }
    .${system}
    or (throw "moshi-hook: unsupported system ${system}");
  asset = assetFor stdenv.hostPlatform.system;
in
stdenv.mkDerivation {
  pname = "moshi-hook";
  inherit version;
  src = fetchurl {
    url = "https://cdn.getmoshi.app/hook/${version}/moshi-hook_${asset.name}.tar.gz";
    hash = asset.hash;
  };

  sourceRoot = ".";

  installPhase = ''
    runHook preInstall
    install -Dm755 moshi-hook $out/bin/moshi-hook
    ln -s moshi-hook $out/bin/moshi
    runHook postInstall
  '';

  meta = with lib; {
    description = "Moshi companion daemon for agent hooks and mobile approvals";
    homepage = "https://getmoshi.app/";
    license = licenses.unfree;
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
    mainProgram = "moshi-hook";
  };
}
