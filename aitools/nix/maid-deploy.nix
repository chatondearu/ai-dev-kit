# Maid file.* fragments for cda.aitools config deploy (imported by module).
{
  lib,
  pkgs,
  templates,
  projectsRoot,
  tmuxClipboard,
  tmuxPersist,
  navBind ? {enable = false;},
}:
let
  configToml =
    pkgs.writeText "cdamux-config.toml" (
      builtins.replaceStrings ["@PROJECTS_ROOT@"] [projectsRoot] (
        builtins.readFile "${templates}/cdamux/config.toml"
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

  resurrectPlugin = "${pkgs.tmuxPlugins.resurrect}/share/tmux-plugins/resurrect";
  continuumPlugin = "${pkgs.tmuxPlugins.continuum}/share/tmux-plugins/continuum";

  persistBody =
    builtins.replaceStrings ["@CONTINUUM_SAVE_INTERVAL@"] [(toString tmuxPersist.saveInterval)] (
      builtins.readFile "${templates}/tmux-persist.conf"
    )
    + "\n"
    + "run-shell '${resurrectPlugin}/resurrect.tmux'\n"
    + "run-shell '${continuumPlugin}/continuum.tmux'\n";

  navBindBody =
    if navBind.enable then
      "\n\n" + builtins.readFile "${templates}/tmux-nav-bind.conf"
    else
      "";

  baseBody = builtins.readFile "${templates}/tmux.conf" + navBindBody;

  tmuxConfSource =
    if tmuxPersist.enable then
      pkgs.writeText "tmux.conf" (baseBody + "\n\n" + persistBody)
    else if navBind.enable then
      pkgs.writeText "tmux.conf" baseBody
    else
      "${templates}/tmux.conf";
in {
  file.home.".tmux.conf".text = ''
    # Managed by cda.aitools (ai-dev-kit)
    source-file ~/.config/cda-aitools/tmux.conf
    ${tmuxHomeExtra}
  '';

  file.xdg_config."cda-aitools/tmux.conf".source = tmuxConfSource;
  file.xdg_config."cdamux/config.toml".source = configToml;
}
