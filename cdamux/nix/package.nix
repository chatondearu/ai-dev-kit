# cdamux CLI + cdamuxd (in-tree; used by cda.aitools Nix modules).
{
  lib,
  rustPlatform,
  pkg-config,
}:
rustPlatform.buildRustPackage {
  pname = "cdamux";
  version = "0.0.1";

  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [pkg-config];

  # Integration tests expect tmux on PATH; package build only.
  doCheck = false;

  meta = with lib; {
    description = "AI agent session orchestration on tmux";
    homepage = "https://github.com/chatondearu/ai-dev-kit/tree/main/cdamux";
    license = licenses.mit;
    mainProgram = "cdamux";
  };
}
