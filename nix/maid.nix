# nix-maid file declarations for this kit.
#
# The shared SKILL.md skills are linked into every target in `skillDirs`
# (one per agent: Cursor, Claude, opencode, …). Cursor-specific assets
# (user rules, subagents, local plugins) are linked into ~/.cursor only.
#
# Entries are derived from the repo tree at eval time, so new assets are picked
# up without editing this file. `repoPath` uses nix-maid's `{{home}}` mustache
# so symlinks target the live clone (edit without rebuilding).
{
  repoPath ? "{{home}}/dev/chatondearu/ai-dev-kit",
  # Home-relative skill destinations, one per agent you want to feed.
  skillDirs ? [ ".cursor/skills" ],
}:

let
  childrenOf = rel:
    let dir = ./. + "/../${rel}";
    in if builtins.pathExists dir
       then builtins.attrNames (builtins.readDir dir)
       else [ ];

  # name/value pair: <homeDir>/<name> -> <repoPath>/<srcRel>/<name>
  mk = homeDir: srcRel: name: {
    name = "${homeDir}/${name}";
    value.source = "${repoPath}/${srcRel}/${name}";
  };

  # Shared skills fanned out to each agent destination.
  skillFiles = builtins.concatMap
    (homeDir: map (mk homeDir "skills") (childrenOf "skills"))
    skillDirs;

  cursorRuleFiles = map (mk ".cursor/user-rules" "rules") (childrenOf "rules");
  cursorAgentFiles = map (mk ".cursor/agents" "agents") (childrenOf "agents");
  cursorPluginFiles = map (mk ".cursor/plugins/local" "cursor/plugins/local")
    (childrenOf "cursor/plugins/local");

  all = skillFiles ++ cursorRuleFiles ++ cursorAgentFiles ++ cursorPluginFiles;
in
{
  file.home = builtins.listToAttrs all;
}
