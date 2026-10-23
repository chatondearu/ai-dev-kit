# Maid file.* fragments for cda.aitools config deploy (imported by module).
{
  lib,
  pkgs,
  templates,
  projectsRoot,
  tmuxClipboard,
}:
let
  configToml =
    pkgs.writeText "ccmux-config.toml" (
      builtins.replaceStrings ["@PROJECTS_ROOT@"] [projectsRoot] (
        builtins.readFile "${templates}/ccmux/config.toml"
      )
    );

  tmuxHomeExtra =
    {
      darwin = "";
      wayland = ''
        bind -T copy-mode MouseDragEnd1Pane send-keys -X copy-pipe-and-cancel "wl-copy -n"
        bind -T copy-mode-vi MouseDragEnd1Pane send-keys -X copy-pipe-and-cancel "wl-copy -n"
        bind -T copy-mode-vi y send-keys -X copy-pipe-and-cancel "wl-copy -n"
      '';
      none = "";
    }
    .${tmuxClipboard};
in {
  file.home.".tmux.conf".text = ''
    # Managed by cda.aitools (ai-dev-kit)
    source-file ~/.config/cda-aitools/tmux.conf
    ${tmuxHomeExtra}
  '';

  file.xdg_config."cda-aitools/tmux.conf".source = "${templates}/tmux.conf";
  file.xdg_config."cda-aitools/ccmux/ccmux.json".source = "${templates}/ccmux/ccmux.json";
  file.xdg_config."ccmux/ccmux.json".source = "${templates}/ccmux/ccmux.json";
  file.xdg_config."ccmux/config.toml".source = configToml;
}
