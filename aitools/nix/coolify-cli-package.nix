# Official Coolify CLI (coollabsio/coolify-cli) — pinned GitHub release binaries.
# Opt-in via flake packages.coolify-cli and cda.aitools.coolify (ADR-001 / ADR-003).
{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  makeWrapper,
}:
let
  version = "1.11.0";
  assets = {
    "x86_64-linux" = {
      pair = "linux_amd64";
      hash = "sha256-xk7QJnXel6+iZa2t7zSfFbztvR5SowqL5w5Q2cZJpRc=";
    };
    "aarch64-linux" = {
      pair = "linux_arm64";
      hash = "sha256-Eg+k1YeNTyqK7Efq8r4mSr+dSx28eG5siva541VEBxk=";
    };
    "x86_64-darwin" = {
      pair = "darwin_amd64";
      hash = "sha256-lfQISq2W8G2bTFfPyYYSsoBcznEG6iqBTg2Pra2Pg+o=";
    };
    "aarch64-darwin" = {
      pair = "darwin_arm64";
      hash = "sha256-Sj5LjsKNT36YK89hUBKDZpnpRmL3LloQxqCmNM4lTDI=";
    };
  };
  system = stdenv.hostPlatform.system;
  asset =
    assets.${system}
      or (throw "coolify-cli: unsupported system ${system}");
in
stdenv.mkDerivation {
  pname = "coolify-cli";
  inherit version;

  src = fetchurl {
    url = "https://github.com/coollabsio/coolify-cli/releases/download/v${version}/coolify-cli_${version}_${asset.pair}.tar.gz";
    inherit (asset) hash;
  };

  nativeBuildInputs =
    [makeWrapper]
    ++ lib.optionals stdenv.isLinux [autoPatchelfHook];

  sourceRoot = ".";

  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin
    install -m755 coolify $out/bin/coolify
    # Bootstrap ~/.config/coolify from host env (token never in the Nix store).
    cat > $out/bin/coolify-env-sync <<'EOF'
    #!/bin/sh
    set -eu
    base="''${COOLIFY_BASE_URL:-}"
    token="''${COOLIFY_TOKEN:-}"
    name="''${COOLIFY_CONTEXT:-default}"
    if [ -z "$base" ] || [ -z "$token" ]; then
      echo "coolify-env-sync: set COOLIFY_BASE_URL and COOLIFY_TOKEN (optional COOLIFY_CONTEXT)" >&2
      exit 2
    fi
    coolify context add -d -f "$name" "$base" "$token"
    coolify context verify
    EOF
    chmod +x $out/bin/coolify-env-sync
    wrapProgram $out/bin/coolify-env-sync --prefix PATH : $out/bin
    runHook postInstall
  '';

  meta = with lib; {
    description = "Official Coolify CLI (pinned GitHub release)";
    homepage = "https://github.com/coollabsio/coolify-cli";
    license = licenses.mit;
    mainProgram = "coolify";
    platforms = builtins.attrNames assets;
  };
}
