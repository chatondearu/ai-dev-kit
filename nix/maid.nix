# nix-maid file declarations for this kit's Cursor assets.
#
# Returns a `maid`-compatible attrset (`{ file.home = { ... }; }`). The set of
# linked entries is derived from the repo's `cursor/` tree at eval time, so a
# new skill/rule/agent/plugin is picked up without editing this file.
#
# `repoPath` uses nix-maid's `{{home}}` mustache so the symlink targets the live
# clone at runtime (edit skills without rebuilding). Override it to match where
# the repo is checked out on the machine.
{ repoPath ? "{{home}}/dev/chatondearu/ai-dev-kit" }:

let
  # Parent dirs whose immediate children are linked individually, so files from
  # other tools living next to ours under ~/.cursor are never clobbered.
  subdirs = [ "skills" "user-rules" "agents" "plugins/local" ];

  childrenOf = sub:
    let dir = ./. + "/../cursor/${sub}";
    in if builtins.pathExists dir
       then builtins.attrNames (builtins.readDir dir)
       else [ ];

  entriesFor = sub: map (name: { inherit sub name; }) (childrenOf sub);
  allEntries = builtins.concatLists (map entriesFor subdirs);

  toFile = e: {
    name = ".cursor/${e.sub}/${e.name}";
    value.source = "${repoPath}/cursor/${e.sub}/${e.name}";
  };
in
{
  file.home = builtins.listToAttrs (map toFile allEntries);
}
