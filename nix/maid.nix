# nix-maid file declarations for this kit.
#
# Shared SKILL.md skills are linked into every target in `skillDirs`
# (flattened: any directory under skills/ that contains SKILL.md).
# Cursor-specific assets → ~/.cursor only.
# Claude: claude/CLAUDE.md → ~/.claude/CLAUDE.md (run assemble-claude-md.sh first).
#
# Entries are derived from the repo tree at eval time.
{
  repoPath ? "{{home}}/dev/chatondearu/ai-dev-kit",
  skillDirs ? [ ".cursor/skills" ],
}:

let
  repoRoot = ./. + "/..";

  readDirOrEmpty = path:
    if builtins.pathExists path then builtins.readDir path else { };

  childrenOf = rel:
    builtins.attrNames (readDirOrEmpty (repoRoot + "/${rel}"));

  # Directories under skills/ (any depth) that contain SKILL.md
  findSkillDirs = baseRel:
    let
      basePath = repoRoot + "/${baseRel}";
      entries = readDirOrEmpty basePath;
    in builtins.concatLists (
      builtins.map (name:
        let
          subRel = "${baseRel}/${name}";
          subPath = repoRoot + "/${subRel}";
          subEntries = readDirOrEmpty subPath;
        in
          if subEntries ? "SKILL.md"
          then [ { inherit name; srcRel = subRel; } ]
          else findSkillDirs subRel
      ) (builtins.attrNames entries)
    );

  mk = homeDir: srcRel: name: {
    name = "${homeDir}/${name}";
    value.source = "${repoPath}/${srcRel}/${name}";
  };

  mkSkill = homeDir: { name, srcRel }: {
    name = "${homeDir}/${name}";
    value.source = "${repoPath}/${srcRel}";
  };

  skillEntries = findSkillDirs "skills";

  skillFiles = builtins.concatMap
    (homeDir: map (mkSkill homeDir) skillEntries)
    skillDirs;

  cursorRuleFiles = map (mk ".cursor/user-rules" "rules") (childrenOf "rules");
  cursorAgentFiles = map (mk ".cursor/agents" "agents") (childrenOf "agents");
  cursorPluginFiles = map (mk ".cursor/plugins/local" "cursor/plugins/local")
    (childrenOf "cursor/plugins/local");

  claudeMdFile = {
    name = ".claude/CLAUDE.md";
    value.source = "${repoPath}/claude/CLAUDE.md";
  };

  all = skillFiles ++ cursorRuleFiles ++ cursorAgentFiles ++ cursorPluginFiles
    ++ [ claudeMdFile ];
in
{
  file.home = builtins.listToAttrs all;
}
