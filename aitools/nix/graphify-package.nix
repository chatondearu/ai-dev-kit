# graphifyy (CLI: graphify) — pinned PyPI wheel for cda.aitools.graphify.
# sdist needs setuptools>=83 (not in current nixpkgs); wheel install avoids that.
# Many tree-sitter grammars are missing from nixpkgs; those deps are relaxed.
# Prefer installMethod = "auto" / "uv" when you need full language coverage.
{
  lib,
  python3Packages,
  fetchurl,
}:
python3Packages.buildPythonApplication rec {
  pname = "graphifyy";
  version = "0.9.102";
  format = "wheel";

  src = fetchurl {
    url = "https://files.pythonhosted.org/packages/13/3e/d155237ec052ad461406d2d67f4af681eee00682882831e37c965442d035/graphifyy-0.9.102-py3-none-any.whl";
    hash = "sha256-7iPOsD7ntHxiMLosFvAukosRQ2Zg1anUqMFPiL+PIUs=";
  };

  nativeBuildInputs = with python3Packages; [pythonRelaxDepsHook];

  # shortcut: nixpkgs lacks several tree-sitter-* wheels graphifyy declares;
  # relax them so the Nix path builds; use installMethod=uv for full grammars.
  pythonRelaxDeps = true;
  pythonRemoveDeps = [
    "tree-sitter-typescript"
    "tree-sitter-go"
    "tree-sitter-java"
    "tree-sitter-groovy"
    "tree-sitter-c"
    "tree-sitter-cpp"
    "tree-sitter-ruby"
    "tree-sitter-c-sharp"
    "tree-sitter-kotlin"
    "tree-sitter-scala"
    "tree-sitter-php"
    "tree-sitter-swift"
    "tree-sitter-lua"
    "tree-sitter-zig"
    "tree-sitter-powershell"
    "tree-sitter-elixir"
    "tree-sitter-objc"
    "tree-sitter-julia"
    "tree-sitter-verilog"
    "tree-sitter-fortran"
  ];

  dependencies = with python3Packages; [
    networkx
    numpy
    rapidfuzz
    pyyaml
    tomli
    tree-sitter
    tree-sitter-python
    tree-sitter-javascript
    tree-sitter-rust
    tree-sitter-json
    tree-sitter-bash
  ];

  doCheck = false;

  pythonImportsCheck = ["graphify"];

  meta = with lib; {
    description = "Graphify code-graph CLI (PyPI graphifyy)";
    homepage = "https://github.com/Graphify-Labs/graphify";
    license = licenses.asl20;
    mainProgram = "graphify";
    platforms = platforms.unix;
  };
}
