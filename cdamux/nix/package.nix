# cdamux CLI + cdamuxd (in-tree; used by cda.aitools Nix modules).
# Also ships the agent hooks under share/cdamux/hooks (+ a `node` link, so hook
# commands never need a host node) and bin/cdamux-hooks to wire them.
{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  nodejs,
  jq,
  coreutils,
  runtimeShell,
}:
rustPlatform.buildRustPackage {
  pname = "cdamux";
  version = "0.0.1";

  src = lib.cleanSource ../.;
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [pkg-config makeWrapper];

  # Integration tests expect tmux on PATH; package build only.
  doCheck = false;

  postInstall = ''
    hooks=$out/share/cdamux/hooks
    mkdir -p $hooks
    cp hooks/*.js hooks/cdamux-hooks $hooks/
    substituteInPlace $hooks/cdamux-hooks --replace-fail '#!/usr/bin/env bash' '#!${runtimeShell}'
    ln -s ${nodejs}/bin/node $hooks/node
    makeWrapper $hooks/cdamux-hooks $out/bin/cdamux-hooks \
      --prefix PATH : ${lib.makeBinPath [jq coreutils]}
  '';

  meta = with lib; {
    description = "AI agent session orchestration on tmux";
    homepage = "https://github.com/chatondearu/ai-dev-kit/tree/main/cdamux";
    license = licenses.mit;
    mainProgram = "cdamux";
  };
}
